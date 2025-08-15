use reqwest::Client;

#[tokio::test]
async fn test_simple() {
    let client = Client::new();
    let res = client
        .post("http://127.0.0.1:5999")
        .json(&serde_json::json!({
            "bundle": []
        }))
        .send()
        .await
        .unwrap()
        .status();
    panic!("{res}")
}
