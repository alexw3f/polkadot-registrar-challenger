use crate::comms::{CommsMessage, CommsVerifier};
use crate::identity::{AccountState, OnChainIdentity};
use crate::primitives::{Account, AccountType, Judgement, NetAccount, NetworkAddress, Result};
use futures::{select_biased, FutureExt};
use serde_json::Value;
use std::convert::TryFrom;
use std::result::Result as StdResult;
use websockets::{Frame, WebSocket};

#[derive(Debug, Clone, Serialize, Deserialize)]
enum EventType {
    #[serde(rename = "ack")]
    Ack,
    #[serde(rename = "error")]
    Error,
    #[serde(rename = "newJudgementRequest")]
    NewJudgementRequest,
    #[serde(rename = "judgementResult")]
    JudgementResult,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Message {
    event: EventType,
    data: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AckResponse {
    result: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ErrorResponse {
    error: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct JudgementResponse {
    address: NetAccount,
    judgement: Judgement,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct JudgementRequest {
    address: NetAccount,
    accounts: Accounts,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Accounts {
    display_name: Option<String>,
    legal_name: Option<String>,
    email: Option<Account>,
    web: Option<Account>,
    twitter: Option<Account>,
    #[serde(rename = "riot")]
    matrix: Option<Account>,
}

pub struct Connector {
    client: WebSocket,
    comms: CommsVerifier,
}

#[derive(Debug, Fail)]
enum ConnectorError {
    #[fail(display = "the received message is invalid: {}", 0)]
    InvalidMessage(failure::Error),
    #[fail(display = "failed to respond: {}", 0)]
    Response(failure::Error),
    #[fail(display = "failed to fetch messages from the listener: {}", 0)]
    Receiver(failure::Error),
}

impl Connector {
    pub async fn new(url: &str, comms: CommsVerifier) -> Result<Self> {
        let mut connector = Connector {
            client: WebSocket::connect(url).await?,
            comms: comms,
        };

        connector.send_ack(Some("Connection established")).await?;

        Ok(connector)
    }
    async fn send_ack(&mut self, msg: Option<&str>) -> StdResult<(), ConnectorError> {
        self.client
            .send_text(
                serde_json::to_string(&Message {
                    event: EventType::Ack,
                    data: serde_json::to_value(&AckResponse {
                        result: msg
                            .map(|txt| txt.to_string())
                            .unwrap_or("Message acknowledged".to_string()),
                    })
                    .map_err(|err| ConnectorError::Response(err.into()))?,
                })
                .map_err(|err| ConnectorError::Response(err.into()))?,
                false,
                true,
            )
            .await
            .map_err(|err| ConnectorError::Response(err.into()))
            .map(|_| ())
    }
    async fn send_error(&mut self) -> StdResult<(), ConnectorError> {
        self.client
            .send_text(
                serde_json::to_string(&Message {
                    event: EventType::Error,
                    data: serde_json::to_value(&ErrorResponse {
                        error: "Message is invalid. Rejected".to_string(),
                    })
                    .map_err(|err| ConnectorError::Response(err.into()))?,
                })
                .map_err(|err| ConnectorError::Response(err.into()))?,
                false,
                true,
            )
            .await
            .map_err(|err| ConnectorError::Response(err.into()))
            .map(|_| ())
    }
    pub async fn start(mut self) {
        let mut receiver_error = false;

        loop {
            let _ = self.local().await.map_err(|err| {
                match err {
                    ConnectorError::Receiver(err) => {
                        // Prevent spamming log messages if the server is
                        // disconnected.
                        if !receiver_error {
                            error!("Disconnected from Listener: {}", err);
                            receiver_error = true;
                        }
                    }
                    _ => {
                        receiver_error = false;

                        error!("{}", err);
                    }
                }
            });
        }
    }
    async fn local(&mut self) -> StdResult<(), ConnectorError> {
        use EventType::*;

        select_biased! {
            msg = self.comms.recv().fuse() => {
                match msg {
                    CommsMessage::JudgeIdentity {
                        network_address,
                        judgement,
                    } => {
                        self.client
                            .send_text(
                                serde_json::to_string(&Message {
                                    event: EventType::JudgementResult,
                                    data: serde_json::to_value(&JudgementResponse {
                                        address: network_address.address().clone(),
                                        judgement: judgement,
                                    })
                                    .map_err(|err| ConnectorError::Response(err.into()))?,
                                })
                                .map_err(|err| ConnectorError::Response(err.into()))?,
                                false,
                                true,
                            )
                            .await
                            .map_err(|err| ConnectorError::Response(err.into()))?;
                    }
                    _ => panic!("Received invalid message in Connector"),
                }
            },
            frame = self.client.receive().fuse() => {
                match frame.map_err(|err| ConnectorError::Receiver(err.into()))? {
                    Frame::Text { payload, .. } => {
                        let try_msg = serde_json::from_str::<Message>(&payload);
                        let msg = if let Ok(msg) = try_msg {
                            msg
                        } else {
                            self.send_error().await?;
                            return Err(ConnectorError::InvalidMessage(try_msg.unwrap_err().into()));
                        };

                        println!("DEBUG MSG: {:?}", msg);

                        match msg.event {
                            NewJudgementRequest => {
                                println!("Received a new identity from Watcher!");
                                if let Ok(request) =
                                    serde_json::from_value::<JudgementRequest>(msg.data)
                                {
                                    let ident = if let Ok(ident) = OnChainIdentity::try_from(request) {
                                        self.send_ack(None).await?;

                                        self.comms.notify_new_identity(ident);
                                    } else {
                                        self.send_error().await?;
                                    };
                                } else {
                                    self.send_error().await?;
                                }
                            }
                            _ => {}
                        }
                    }
                    _ => {
                        self.send_error().await?;
                    }
                }
            }
        };

        Ok(())
    }
}

impl TryFrom<JudgementRequest> for OnChainIdentity {
    type Error = failure::Error;

    fn try_from(request: JudgementRequest) -> Result<Self> {
        let accs = request.accounts;

        Ok(OnChainIdentity {
            network_address: NetworkAddress::try_from(request.address)?,
            display_name: accs.display_name,
            legal_name: accs.legal_name,
            email: accs
                .email
                .map(|v| AccountState::new(Account::from(v), AccountType::Email)),
            web: accs
                .web
                .map(|v| AccountState::new(Account::from(v), AccountType::Web)),
            twitter: accs
                .twitter
                .map(|v| AccountState::new(Account::from(v), AccountType::Twitter)),
            matrix: accs
                .matrix
                .map(|v| AccountState::new(Account::from(v), AccountType::Matrix)),
        })
    }
}