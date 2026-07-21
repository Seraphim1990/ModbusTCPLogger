use tokio::sync::oneshot;


#[derive(Clone, Default, sqlx::FromRow, serde::Serialize, serde::Deserialize, Debug)]
#[derive(PartialEq)]
pub struct HashedValue {
    pub val: f64,
    pub timestamp: i64,
}
#[derive(Clone, serde::Serialize, Debug)]
pub struct MeasureResponse {
    pub id: i32,
    pub values: Vec<HashedValue>,
    pub from: i64,
    pub to: i64,
}
#[derive(Debug)]
pub struct MeasureRequest {
    pub from: i64,
    pub to: i64,
    pub values_id: Vec<i32>,
    pub response_sender: oneshot::Sender<Result<Vec<MeasureResponse>, String>>,
}