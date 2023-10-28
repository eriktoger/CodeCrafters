mod utils;
use std::{
    fs,
    io::Write,
    net::{TcpListener, TcpStream},
    path::Path,
    thread,
};
use utils::{get_directory, get_filename_from_path, get_lines};

struct Response {
    status_code: String,
    body: Option<String>,
    content_type: String,
}

fn main() {
    let listener = TcpListener::bind("127.0.0.1:4221").unwrap();

    for stream in listener.incoming() {
        match stream {
            Ok(mut stream) => {
                thread::spawn(move || {
                    let response =
                        parse_request(stream.try_clone().expect("failed to clone stream"));
                    let response = generate_response(response);
                    let _ = stream.write_all(response.as_bytes());

                    stream.flush().expect("failed to flush the stream");
                });
            }
            Err(e) => {
                println!("error: {}", e);
            }
        }
    }
}

fn parse_request(stream: TcpStream) -> Response {
    let lines = get_lines(stream);

    let path = lines[1].to_string();
    let method = lines[0].to_string();

    if method == "POST" {
        return handle_post(lines, path);
    }

    handle_get(lines, path)
}

fn generate_response(response: Response) -> String {
    let status_code = response.status_code;
    match response.body {
        Some(b) => {
            let content_length = b.len();
            let content_type = response.content_type;
            format!(
            "HTTP/1.1 {status_code}\r\nContent-Type: {content_type}\r\nContent-Length: {content_length}\r\n\r\n{b}\r\n",

        )
        }
        None => format!("HTTP/1.1 {status_code} OK\r\n\r\n"),
    }
}

fn handle_post(lines: Vec<String>, path: String) -> Response {
    if path.starts_with("/files") {
        let dir = get_directory().expect("a dir");
        let filename = get_filename_from_path(path);
        let file_path = Path::new(&dir).join(filename);

        let empty_index = lines
            .iter()
            .position(String::is_empty)
            .expect("to have empty item");
        let content = lines
            .iter()
            .skip(empty_index + 1)
            .map(String::to_string)
            .collect::<Vec<String>>()
            .join(" ");
        let _ = fs::write(file_path, content.to_string());

        return Response {
            status_code: "201".to_string(),
            body: None,
            content_type: "text/plain".to_string(),
        };
    }
    return Response {
        status_code: "404".to_string(),
        body: None,
        content_type: "text/plain".to_string(),
    };
}

fn handle_get(lines: Vec<String>, path: String) -> Response {
    if path == "/" {
        create_home_response()
    } else if path.starts_with("/echo/") {
        create_echo_response(path)
    } else if path.starts_with("/user-agent") {
        create_user_agent_response(lines)
    } else if path.starts_with("/files/") {
        create_file_response(path)
    } else {
        create_not_found_response()
    }
}

fn create_home_response() -> Response {
    Response {
        status_code: "200".to_string(),
        body: None,
        content_type: "".to_string(),
    }
}

fn create_echo_response(path: String) -> Response {
    let body = Some(
        path.strip_prefix("/echo/")
            .expect("to have /echo/-prefix")
            .to_string(),
    );
    Response {
        status_code: "200".to_string(),
        body,
        content_type: "text/plain".to_string(),
    }
}

fn create_user_agent_response(lines: Vec<String>) -> Response {
    let user_agent_index = lines
        .iter()
        .position(|s| *s == "User-Agent:")
        .expect("to have user-agent");
    let user_agent_message = lines[user_agent_index + 1].to_string();
    let body = Some(user_agent_message);
    Response {
        status_code: "200".to_string(),
        body,
        content_type: "text/plain".to_string(),
    }
}

fn create_file_response(path: String) -> Response {
    let filename = get_filename_from_path(path);
    let dir = get_directory().expect("a dir");
    let file_path = Path::new(&dir).join(filename);
    match fs::read_to_string(file_path) {
        Ok(file) => Response {
            status_code: "200".to_string(),
            body: Some(file),
            content_type: "application/octet-stream".to_string(),
        },
        Err(_) => create_not_found_response(),
    }
}

fn create_not_found_response() -> Response {
    Response {
        status_code: "404".to_string(),
        body: None,
        content_type: "".to_string(),
    }
}
