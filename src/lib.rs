use reqwest::Client;
use reqwest::header::{HeaderMap, HeaderValue};
use serde::Deserialize;
use tokio::time::{Duration, sleep};

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum PosylkaAnswerEnum {
    Error(Error),
    Success(PosylkaAnswer),
}
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum AddPosylkaToCabinetAnswerEnum {
    Error(Error),
    Success(AddPosylkaAnswer),
}

#[derive(Debug, Deserialize)]
struct Error {
    status: u16,
    error: String,
}
#[derive(Debug, Deserialize)]
struct PosylkaCode {
    code: String,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Attributes {
    #[serde(default)]
    pub recipient: String,
    #[serde(default)]
    pub estimated_delivery: String,
}
#[derive(Debug, Deserialize)]
pub struct PosylkaAnswer {
    pub attributes: Attributes,
    pub events: Vec<PosylkaEvent>,
    #[serde(default)]
    pub delivered: bool,
}
#[derive(Debug, Deserialize)]
struct AddPosylkaAnswer {
    result: String,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PosylkaEvent {
    pub event_date: i64,
    pub operation: String,
    #[serde(default)]
    pub location: String,
}
static API_URL: &str = "https://moyaposylka.ru/api/v1/";
static CARRIERS: &str = "carriers/";
static TRACKERS: &str = "trackers/";
#[derive(Default)]
pub struct Moyaposylka {
    pub apikey: String,
    pub client: Client,
}
impl Moyaposylka {
    pub fn new(apikey: String) -> Moyaposylka {
        Moyaposylka {
            apikey,
            client: Client::new(),
        }
    }
    async fn get_posylka_response(
        &self,
        url: &str,
    ) -> Result<PosylkaAnswerEnum, Box<dyn std::error::Error>> {
        let resp = self.client.get(url).send().await?;
        let posylka_answer: PosylkaAnswerEnum = resp.json().await?;
        Ok(posylka_answer)
    }

    pub async fn get_posylka(
        &self,
        track: &str,
    ) -> Result<PosylkaAnswer, Box<dyn std::error::Error>> {
        if track.len() < 7 {
            return Err(format!("Введён несуществующий трек-номер {}", track).into());
        }
        let carriers_response = self
            .client
            .get(format!("{}{}{}", API_URL, CARRIERS, track))
            .send()
            .await?;
        if carriers_response.status().is_success() {
            let carriers: Vec<PosylkaCode> = carriers_response.json::<Vec<PosylkaCode>>().await?;
            if carriers.is_empty() {
                Err(format!("Нет данных о трек-номере: {}", track).into())
            } else {
                let posylka_answer: PosylkaAnswerEnum = self
                    .get_posylka_response(&format!(
                        "{}{}{}/{}",
                        API_URL, TRACKERS, carriers[0].code, track
                    ))
                    .await?;
                match posylka_answer {
                    PosylkaAnswerEnum::Success(answer) => Ok(answer),
                    PosylkaAnswerEnum::Error(error) => {
                        if error.status == 404 {
                            println!("Добавление трек-номера {} в отслеживание", track);
                            let mut headers = HeaderMap::new();
                            headers
                                .insert("X-Api-Key", HeaderValue::from_str(&self.apikey).unwrap());
                            headers.insert(
                                "Content-Type",
                                HeaderValue::from_str("application/json").unwrap(),
                            );
                            let add_tracker_response = self
                                .client
                                .post(&format!(
                                    "{}{}{}/{}",
                                    API_URL, TRACKERS, carriers[0].code, track
                                ))
                                .headers(headers)
                                .send()
                                .await?;
                            if add_tracker_response.status().is_success() {
                                let added_tracker = add_tracker_response.json().await?;
                                match added_tracker {
                                    AddPosylkaToCabinetAnswerEnum::Error(error) => Err(format!(
                                        "Ошибка при добавлении трек-номера в отслеживание: {}",
                                        error.error
                                    )
                                    .into()),
                                    AddPosylkaToCabinetAnswerEnum::Success(add_posylka_answer) => {
                                        if add_posylka_answer.result == "success" {
                                            println!(
                                                "пауза в 5 секунд на добавление трек-номера в отслеживание"
                                            );
                                            sleep(Duration::from_secs(5)).await; //ждём 5 секунд для отслеживания добавленного
                                            let posylka_answer = self
                                                .get_posylka_response(&format!(
                                                    "{}{}{}/{}",
                                                    API_URL, TRACKERS, carriers[0].code, track
                                                ))
                                                .await?;
                                            match posylka_answer {
                                                PosylkaAnswerEnum::Success(answer) => Ok(answer),
                                                PosylkaAnswerEnum::Error(error) => Err(format!(
                                                    "Ошибка при добавлении отслеживания трек-номера {} {}",
                                                    track,
                                                    error.error
                                                )
                                                .into()),
                                            }
                                        } else {
                                            Err(format!("Ошибка при добавлении трек-номера в отслеживание: {}", 
                                            add_posylka_answer.result).into()
                                        )
                                        }
                                    }
                                }
                            } else {
                                Err(format!(
                                    "Ошибка при добавлении трек-номера в отслеживание: {}",
                                    add_tracker_response.status()
                                )
                                .into())
                            }
                        } else {
                            Err(format!("Ошибка при получении данных: {}", error.error).into())
                        }
                    }
                }
            }
        } else {
            Err(format!(
                "Ошибка при получении данных: {} {}",
                carriers_response.status(),
                carriers_response.url()
            )
            .into())
        }
    }
}
