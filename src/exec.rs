use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio_tungstenite::{
    tungstenite::{self, Message},
    MaybeTlsStream, WebSocketStream,
};

use crate::{nomad::allocation, NomadEndpoint};

pub struct ExecSession {
    connection: tokio_util::either::Either<
        WebSocketStream<MaybeTlsStream<TcpStream>>,
        WebSocketStream<tokio::net::UnixStream>,
    >,
}

#[derive(Debug)]
pub enum StartError {
    CommandToJson(serde_json::Error),
    WebsocketHttp { response: String },
    WebsocketConnect(tokio_tungstenite::tungstenite::Error),
    InvalidWebsocketStatus,
}

impl ExecSession {
    pub async fn start(
        endpoint: &NomadEndpoint,
        host: &str,
        port: u16,
        alloc_id: &str,
        task_name: &str,
        cmds: &[&str],
    ) -> Result<Self, StartError> {
        let route = format!("v1/client/allocation/{}/exec", alloc_id);

        let cmd_string = serde_json::to_string(cmds).map_err(|e| StartError::CommandToJson(e))?;
        let command_encoded = urlencoding::encode(&cmd_string);

        let mut ub = url_builder::URLBuilder::new();
        ub.set_protocol("ws")
            .set_host(host)
            .set_port(port)
            .add_route(&route)
            .add_param("command", &command_encoded)
            .add_param("task", task_name)
            .add_param("tty", "false");

        let (ws_connection, response) = match endpoint {
            NomadEndpoint::HTTP { .. } => {
                let ws_url = ub.build();

                match tokio_tungstenite::connect_async(ws_url).await {
                    Ok((c, r)) => (tokio_util::either::Either::Left(c), r),
                    Err(e) => {
                        match e {
                            tokio_tungstenite::tungstenite::Error::Http(resp) => {
                                let body = resp.body().clone().unwrap();
                                let string = String::from_utf8(body).unwrap();

                                return Err(StartError::WebsocketHttp { response: string });
                            }
                            other => return Err(StartError::WebsocketConnect(other)),
                        };
                    }
                }
            }
            NomadEndpoint::UnixSocket { path, token } => {
                let host = ub.host().to_string();
                let ws_url = ub.build();

                let req = tungstenite::handshake::client::Request::builder()
                    .method("GET")
                    .header("Host", host)
                    .header("Connection", "Upgrade")
                    .header("Upgrade", "websocket")
                    .header("Sec-WebSocket-Version", "13")
                    .header("Authorization", format!("Bearer {}", token))
                    .header(
                        "Sec-WebSocket-Key",
                        tungstenite::handshake::client::generate_key(),
                    )
                    .uri(ws_url)
                    .body(())
                    .unwrap();

                let stream = tokio::net::UnixStream::connect(path).await.unwrap();
                match tokio_tungstenite::client_async(req, stream).await {
                    Ok((c, r)) => (tokio_util::either::Either::Right(c), r),
                    Err(e) => {
                        match e {
                            tokio_tungstenite::tungstenite::Error::Http(resp) => {
                                let body = resp.body().clone().unwrap();
                                let string = String::from_utf8(body).unwrap();

                                return Err(StartError::WebsocketHttp { response: string });
                            }
                            other => return Err(StartError::WebsocketConnect(other)),
                        };
                    }
                }
            }
        };

        if !response.status().is_informational() {
            return Err(StartError::InvalidWebsocketStatus);
        }

        Ok(Self {
            connection: ws_connection,
        })
    }

    pub async fn execute_command<SO, SE>(
        &mut self,
        command: &str,
        stdout: SO,
        stderr: SE,
    ) -> Result<i32, ()>
    where
        SO: FnMut(String),
        SE: FnMut(String),
    {
        let request = allocation::ExecRequestFrame {
            stdin: allocation::ExecData::data(&format!("{}\n", command)),
        };

        self.connection
            .send(Message::Text(
                serde_json::to_string(&request).unwrap().into(),
            ))
            .await
            .unwrap();

        self.read_logs(stdout, stderr).await
    }

    pub async fn read_logs<SO, SE>(&mut self, mut stdout: SO, mut stderr: SE) -> Result<i32, ()>
    where
        SO: FnMut(String),
        SE: FnMut(String),
    {
        let mut exit_code = 0;

        while let Some(msg_res) = self.connection.next().await {
            let msg = match msg_res {
                Ok(m) => m,
                Err(e) => {
                    // FIXME
                    return Ok(0);
                    panic!("Error: {:?}", e);
                }
            };

            let frame: allocation::ExecResponseFrame = match msg {
                Message::Text(txt) => serde_json::from_str(&txt).unwrap(),
                Message::Close(_close_frame) => {
                    // println!("{:?}", _close_frame);

                    break;
                }
                other => panic!("Unexpected Message: {:?}", other),
            };

            if let Some(tmp) = &frame.stdout {
                stdout(tmp.decode_data());
            }
            if let Some(tmp) = &frame.stderr {
                stderr(tmp.decode_data());
            }

            if let Some(res) = frame.result {
                exit_code = res.exit_code;
            }
        }

        Ok(exit_code)
    }

    pub async fn write_to_file(&mut self, content: &str, path: &str) -> Result<i32, ()> {
        let cmd = format!(
            "echo '{}' > {}; exit 0",
            content.replace('\'', "'\\''"),
            path
        );

        self.execute_command(&cmd, |_| {}, |_| {}).await
    }
}
