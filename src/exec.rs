use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio_tungstenite::{tungstenite::Message, MaybeTlsStream, WebSocketStream};

use log::error;

use crate::nomad::allocation;

pub struct ExecSession {
    connection: WebSocketStream<MaybeTlsStream<TcpStream>>,
}

impl ExecSession {
    pub async fn start(
        host: &str,
        port: u16,
        alloc_id: &str,
        task_name: &str,
        cmds: &[&str],
    ) -> Result<Self, ()> {
        let route = format!("v1/client/allocation/{}/exec", alloc_id);

        let cmd_string = serde_json::to_string(cmds).unwrap();
        let command_encoded = urlencoding::encode(&cmd_string);

        let mut ub = url_builder::URLBuilder::new();
        ub.set_protocol("ws")
            .set_host(host)
            .set_port(port)
            .add_route(&route)
            .add_param("command", &command_encoded)
            .add_param("task", task_name)
            .add_param("tty", "false");
        let ws_url = ub.build();

        let (ws_connection, response) = match tokio_tungstenite::connect_async(ws_url).await {
            Ok(c) => c,
            Err(e) => {
                match e {
                    tokio_tungstenite::tungstenite::Error::Http(resp) => {
                        let body = resp.body().clone().unwrap();
                        let string = String::from_utf8(body.to_vec()).unwrap();

                        error!("Response: {:?}", string);
                        todo!("{:?}", resp);
                    }
                    other => todo!("{:?}", other),
                };
            }
        };

        if !response.status().is_informational() {
            error!("Websocket Response: {:?}", response);

            todo!("Establishing Websocket Connection was not successful");
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
            .send(Message::Text(serde_json::to_string(&request).unwrap()))
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
