use std::{env, io::Read, net::TcpStream};

pub fn get_lines(mut stream: TcpStream) -> Vec<String> {
    let mut buf = [0; 4096];
    let len = stream.read(&mut buf).unwrap();
    let req = std::str::from_utf8(&buf[0..len]).unwrap().to_string();
    req.replace("\r\n", " ")
        .split(" ")
        .map(str::to_string)
        .collect()
}

pub fn get_directory() -> Option<String> {
    let args: Vec<String> = env::args().collect();
    let directory_index = args.iter().position(|s| *s == "--directory");
    match directory_index {
        Some(index) => {
            if index < (args.len() - 1) {
                Some(args[index + 1].clone())
            } else {
                None
            }
        }
        None => None,
    }
}

pub fn get_filename_from_path(path: String) -> String {
    path.strip_prefix("/files/")
        .expect("to have /files/-prefix")
        .to_string()
}
