use tokio::{
    io::{AsyncBufReadExt as _, AsyncReadExt as _, AsyncWriteExt as _},
    net, task,
};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

pub struct Step {
    pub body: String,
    pub method: String,
    pub path: String,
    pub payload: Option<serde_json::Value>,
    pub status: u16,
}

pub fn get(path: &str, status: u16, body: &str) -> Step {
    Step {
        body: body.to_owned(),
        method: "GET".to_owned(),
        path: path.to_owned(),
        payload: None,
        status,
    }
}

pub fn put(path: &str, payload: serde_json::Value, status: u16, body: &str) -> Step {
    Step {
        body: body.to_owned(),
        method: "PUT".to_owned(),
        path: path.to_owned(),
        payload: Some(payload),
        status,
    }
}

pub async fn serve_all(steps: Vec<Step>) -> (String, task::JoinHandle<Result<(), Error>>) {
    let listener = net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("mock listener");
    let address = listener.local_addr().expect("mock address");
    let base = format!("http://{address}/");
    let task = tokio::spawn(async move {
        for step in steps {
            serve(&listener, &step).await?;
        }
        Ok(())
    });
    (base, task)
}

async fn serve(listener: &net::TcpListener, step: &Step) -> Result<(), Error> {
    let (stream, _) = listener.accept().await?;
    let mut reader = tokio::io::BufReader::new(stream);
    let mut first = String::new();
    reader.read_line(&mut first).await?;
    assert_eq!(
        first.trim(),
        format!("{} /{} HTTP/1.1", step.method, step.path)
    );
    let mut headers = Vec::new();
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).await?;
        if line == "\r\n" {
            break;
        }
        let line = line.to_ascii_lowercase();
        headers.push(line);
    }
    assert!(
        headers
            .iter()
            .any(|header| header.trim() == "authorization: bearer mock-token")
    );
    let length = headers
        .iter()
        .find_map(|header| header.strip_prefix("content-length: "));
    if let Some(length) = length.filter(|length| length.trim() != "0") {
        let length: usize = length.trim().parse().expect("content length");
        let mut body = vec![0; length];
        reader.read_exact(&mut body).await?;
        let body: serde_json::Value = serde_json::from_slice(&body)?;
        assert_eq!(Some(body), step.payload);
    } else {
        assert!(step.payload.is_none());
    }
    let response = format!(
        "HTTP/1.1 {} Mock\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        step.status,
        step.body.len(),
        step.body
    );
    let response = response.as_bytes();
    reader.get_mut().write_all(response).await?;
    Ok(())
}
