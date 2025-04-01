use outro_08::server::{listen, run_server, CreateTicketRequest, CreateTicketResponse, GetTicketResponse};
use futures::future;
use outro_08::data::{Status, Ticket};
use outro_08::store::TicketId;
use std::net::SocketAddr;
use surf;
use surf::Response;
use tide::StatusCode;
use tokio::task::JoinHandle;

struct TestServer(SocketAddr, JoinHandle<Result<(), std::io::Error>>);

impl TestServer {
    pub async fn new() -> TestServer {
        let listener = listen(None).await.unwrap();
        let address = listener.local_addr().unwrap().clone();
        let server = tokio::spawn(run_server(listener));
        TestServer(address, server)
    }

    pub fn address(&self) -> &SocketAddr { &self.0 }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.1.abort();
    }
}

fn create_ticket_request(n: u64) -> CreateTicketRequest {
    CreateTicketRequest {
        title: format!("Title {}", n).to_string(),
        description: format!("Description {}", n).to_string(),
    }
}

async fn create_ticket(address: &SocketAddr, ticket_request: &CreateTicketRequest) -> Response {
    surf::post(format!("http://{}/tickets", address))
        .body_json(&ticket_request).unwrap().await.unwrap()
}

async fn get_ticket(address: &SocketAddr, ticket_id: TicketId) -> Response {
    let uri = format!("http://{}/tickets/{}", address, &ticket_id.0);
    surf::get(uri).await.unwrap()
}


#[tokio::test]
async fn basic_server_functions() {
    let server = TestServer::new().await;

    let create_ticket_req = create_ticket_request(1);

    let mut response = create_ticket(server.address(), &create_ticket_req).await;

    assert_eq!(response.status(), 200);
    let response_body: CreateTicketResponse = response.body_json().await.unwrap();
    assert_eq!(response_body.ticket_id, 0.into());
}

#[tokio::test]
async fn multiple_tickets_are_properly_stored_and_can_be_retrieved() {
    let server = TestServer::new().await;

    async fn create_and_get_ticket(address: &SocketAddr, n: u64) -> () {
        let new_ticket_req = create_ticket_request(n);
        let mut new_ticket_resp = create_ticket(address, &new_ticket_req).await;

        assert_eq!(new_ticket_resp.status(), 200);
        let ticket_id: TicketId = new_ticket_resp.body_json::<CreateTicketResponse>().await.unwrap().ticket_id;

        let mut get_ticket_resp = get_ticket(address, ticket_id).await;
        assert_eq!(get_ticket_resp.status(), 200);
        let retreived_ticket: Ticket = get_ticket_resp.body_json::<GetTicketResponse>().await.unwrap().0;

        assert_eq!(retreived_ticket.title.0, new_ticket_req.title);
        assert_eq!(retreived_ticket.description.0, new_ticket_req.description);
        assert_eq!(retreived_ticket.status, Status::ToDo);
        assert_eq!(retreived_ticket.id, ticket_id);

        ()
    }

    let requests = (1..3)
        .map(|i| create_and_get_ticket(server.address(), i))
        .collect::<Vec<_>>();

    future::join_all(requests).await;
}

#[tokio::test]
async fn malformed_new_ticket_request() {
    let server = TestServer::new().await;

    let response = surf::post(format!("http://{}/tickets", server.address()))
        .body_string("not a json".to_string()).await.unwrap();

    assert_eq!(response.status(), StatusCode::BadRequest);
}

#[tokio::test]
async fn malformed_get_request() {
    let server = TestServer::new().await;

    let ticket_req = CreateTicketRequest {
        title: "Really really long title, so long that it does not really fit in and it will fail validation and everything will blow up".to_string(),
        description: "Description".to_string(),
    };

    let response = create_ticket(server.address(), &ticket_req).await;

    assert_eq!(response.status(), StatusCode::BadRequest);
}

#[tokio::test]
async fn ticket_not_found() {
    let server = TestServer::new().await;

    let response = get_ticket(server.address(), TicketId(333)).await;

    assert_eq!(response.status(), StatusCode::NotFound);
}