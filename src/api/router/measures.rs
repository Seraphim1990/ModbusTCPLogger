use axum::{extract::{Query, State}, http::StatusCode, response::{IntoResponse}, routing::{get}, Json, Router};
use serde::{Deserialize};

use crate::api::init_axum::AppState;
use crate::logger::printers;
use tokio::sync::oneshot;
use crate::messages::requests::{
    measure_request::{MeasureRequest},
    request_struct::Request
};
use crate::api::router::handle_get_request::{check_send_message};
use crate::messages::main_msg::MainMsg;

pub fn measures_router() -> Router<AppState> {
    Router::new()
        .route("/measure/", get(get_measures))
}

#[derive(Debug, Deserialize)]
pub struct MeasureQuery {
    pub value_ids: String,
    pub start_time: i64,
    pub end_time: i64,
}
impl MeasureQuery {
    #[allow(clippy::wrong_self_convention)]
    fn to_request(self) -> Result<GetMeasures, String> {
        let ids: Result<Vec<i32>, _> = self.value_ids
            .split(',')
            .map(|s| s.trim().parse::<i32>())
            .collect();

        let ids = match ids {
            Ok(v) if !v.is_empty() => v,
            _ => {
                let msg = "Невірний формат value_ids".to_string();
                return Err(msg)
            }
        };

        Ok(GetMeasures{
            from: self.start_time,
            to: self.end_time,
            values_id: ids
        })
    }
}
#[derive(Debug, Deserialize)]
struct GetMeasures{
    values_id: Vec<i32>,
    from: i64,
    to: i64,
}

async fn get_measures(State(state): State<AppState>, Query(params): Query<MeasureQuery>) -> impl IntoResponse {
    let request = match params.to_request() {
        Ok(request) => request,
        Err(e) => return (StatusCode::BAD_REQUEST, e).into_response()
    };

    let (tx, rx) = oneshot::channel();
    let request = Request::GetMeasure(
        MeasureRequest{
            from: request.from,
            to: request.to,
            values_id: request.values_id,
            response_sender: tx
        }
    );
    let msg = MainMsg::Request(request);
    if let Err(e) = check_send_message(&state.from_api, msg).await{
        return e.into_response();
    };
    match rx.await {
        Ok(Ok(res)) => {
            (StatusCode::OK, Json(res)).into_response()},
        Ok(Err(_)) => {
            let msg = "Помилка читання бази данних, детальніше в логах".to_string();
            printers::err(msg.clone());
            (StatusCode::INTERNAL_SERVER_ERROR, msg).into_response()
        },
        Err(e) => {
            let msg = format!("Помилка каналу: \n{}", e);
            printers::err(msg.clone());
            (StatusCode::INTERNAL_SERVER_ERROR, msg).into_response()
        }
    }
}

/*
#[derive(Debug, Deserialize)]
pub struct ChartQuery {
    pub value_ids: String,
    pub start_time: i64,
    pub end_time: i64,
}

pub async fn generate_chart(
    State(state): State<AppState>,
    Query(params): Query<ChartQuery>,
) -> impl IntoResponse {
    let ids: Result<Vec<i32>, _> = params.value_ids
        .split(',')
        .map(|s| s.trim().parse::<i32>())
        .collect();

    let ids = match ids {
        Ok(v) if !v.is_empty() => v,
        _ => {
            let msg = "Невірний формат value_ids".to_string();
            printers::err(msg.clone());
            return (StatusCode::BAD_REQUEST, msg).into_response();
        }
    };

    let mut datasets: Vec<serde_json::Value> = vec![];

    let colors = [
        "rgb(255, 99, 132)", "rgb(54, 162, 235)", "rgb(255, 206, 86)",
        "rgb(75, 192, 192)", "rgb(153, 102, 255)", "rgb(255, 159, 64)",
        "rgb(199, 199, 199)", "rgb(83, 102, 255)", "rgb(255, 99, 255)",
        "rgb(99, 255, 132)",
    ];

    for (i, value_id) in ids.iter().enumerate() {
        let value = sqlx::query_as::<_, ValueItem>(
            "SELECT id, name, tag FROM value_items WHERE id = ?"
        )
            .bind(value_id)
            .fetch_optional(&state.pool)
            .await;

        let value = match value {
            Ok(Some(v)) => v,
            Ok(None) => continue,
            Err(e) => {
                printers::err(format!("Помилка читання value_item:\n{}", e));
                continue;
            }
        };

        let measures = sqlx::query_as::<_, Measure>(
            "SELECT id, valueId, measureValue, measureTime FROM measures
             WHERE valueId = ? AND measureTime >= ? AND measureTime <= ?
             ORDER BY measureTime"
        )
            .bind(value_id)
            .bind(params.start_time)
            .bind(params.end_time)
            .fetch_all(&state.pool)
            .await;

        let measures = match measures {
            Ok(m) if !m.is_empty() => m,
            Ok(_) => continue,
            Err(e) => {
                printers::err(format!("Помилка читання measures:\n{}", e));
                continue;
            }
        };

        let color = colors[i % colors.len()];
        let bg = color.replace("rgb", "rgba").replace(')', ", 0.1)");

        datasets.push(serde_json::json!({
            "label": value.name,
            "tag": value.tag.unwrap_or_else(|| "N/A".to_string()),
            "data": measures.iter().map(|m| serde_json::json!({
                "x": m.measure_time * 1000,
                "y": m.measure_value
            })).collect::<Vec<_>>(),
            "borderColor": color,
            "backgroundColor": bg,
            "tension": 0.1,
            "pointRadius": 0,
            "pointHoverRadius": 0,
            "borderWidth": 2
        }));
    }

    if datasets.is_empty() {
        let msg = "Дані за вказаними параметрами не знайдено".to_string();
        return (StatusCode::NOT_FOUND, msg).into_response();
    }

    let html = build_chart_html(&datasets, params.start_time, params.end_time);
    Html(html).into_response()
}

fn format_timestamp(ts: i64) -> String {
    DateTime::from_timestamp(ts, 0)
        .unwrap_or_default()
        .with_timezone(&Local)
        .format("%d.%m.%Y %H:%M")
        .to_string()
}

fn build_chart_html(datasets: &[serde_json::Value], start_time: i64, end_time: i64) -> String {
    let datasets_json = serde_json::to_string(datasets).unwrap_or_default();
    let start_dt = format_timestamp(start_time);
    let end_dt = format_timestamp(end_time);

    format!(r#"<!DOCTYPE html>
<html lang="uk">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Графік вимірів</title>
    <script src="https://cdn.jsdelivr.net/npm/chart.js@4.4.0"></script>
    <script src="https://cdn.jsdelivr.net/npm/chartjs-adapter-date-fns@3.0.0"></script>
    <style>
        * {{ margin: 0; padding: 0; box-sizing: border-box; }}
        body {{ font-family: 'Segoe UI', sans-serif; background: linear-gradient(135deg, #667eea 0%, #764ba2 100%); min-height: 100vh; padding: 20px; }}
        .container {{ max-width: 1800px; margin: 0 auto; background: white; border-radius: 15px; box-shadow: 0 10px 40px rgba(0,0,0,0.2); overflow: hidden; }}
        .header {{ background: linear-gradient(135deg, #667eea 0%, #764ba2 100%); color: white; padding: 30px; text-align: center; }}
        .header h1 {{ font-size: 2.5em; margin-bottom: 10px; }}
        .chart-container {{ padding: 30px 40px; position: relative; height: 70vh; }}
        .info {{ padding: 20px 40px; background: #f8f9fa; border-top: 2px solid #e9ecef; }}
        .legend {{ display: flex; flex-wrap: wrap; gap: 20px; }}
        .legend-item {{ display: flex; align-items: center; gap: 10px; }}
        .legend-color {{ width: 20px; height: 20px; border-radius: 4px; }}
        .print-btn {{ display: inline-block; margin: 20px 40px; padding: 12px 25px; background: #28a745; color: white; border: none; border-radius: 8px; font-weight: 600; cursor: pointer; }}
        .print-btn:hover {{ background: #218838; }}
        @media print {{
            @page {{ size: A4 landscape; margin: 8mm 10mm; }}
            body {{ background: white !important; }}
            .header {{ background: white !important; color: black !important; border-bottom: 1px solid #444; }}
            .chart-container {{ height: 135mm !important; }}
            .print-btn {{ display: none !important; }}
        }}
    </style>
</head>
<body>
    <div class="container">
        <div class="header">
            <h1>📊 Графік вимірів</h1>
            <p>Часовий проміжок: {start_dt} - {end_dt}</p>
        </div>
        <div class="chart-container">
            <canvas id="myChart"></canvas>
        </div>
        <div class="info">
            <h3>📈 Відображені значення:</h3>
            <div class="legend" id="legend"></div>
        </div>
        <button onclick="window.print()" class="print-btn">🖨️ Друк / Зберегти як PDF</button>
    </div>
    <script>
        const datasets = {datasets_json};
        const ctx = document.getElementById('myChart').getContext('2d');
        new Chart(ctx, {{
            type: 'line',
            data: {{ datasets }},
            options: {{
                responsive: true,
                maintainAspectRatio: false,
                interaction: {{ mode: 'nearest', intersect: false, axis: 'x' }},
                plugins: {{
                    legend: {{ position: 'top' }},
                    tooltip: {{
                        callbacks: {{
                            title: ctx => new Date(ctx[0].parsed.x).toLocaleString('uk-UA', {{ timeZone: 'Europe/Kiev' }}),
                            label: ctx => ctx.dataset.label + ': ' + ctx.parsed.y.toFixed(2)
                        }}
                    }}
                }},
                scales: {{
                    x: {{
                        type: 'time',
                        time: {{ unit: 'hour', displayFormats: {{ hour: 'HH:mm', day: 'dd.MM' }} }},
                        title: {{ display: true, text: 'Час' }},
                        ticks: {{ maxRotation: 45, autoSkip: true, maxTicksLimit: 16 }}
                    }},
                    y: {{
                        title: {{ display: true, text: 'Значення' }},
                        ticks: {{ precision: 1, maxTicksLimit: 12 }}
                    }}
                }}
            }}
        }});
        const legendDiv = document.getElementById('legend');
        datasets.forEach(d => {{
            legendDiv.innerHTML += `<div class="legend-item"><div class="legend-color" style="background:${{d.borderColor}}"></div><span><strong>${{d.label}}</strong> (${{d.data.length}} точок)</span></div>`;
        }});
    </script>
</body>
</html>"#)
}


 */