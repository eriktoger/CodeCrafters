use std::{
    fs::File,
    io::{Cursor, Seek, SeekFrom},
};

use anyhow::Error;
use flate2::read::GzDecoder;
use reqwest::blocking;
use serde::Deserialize;
use tar::Archive;

#[derive(Deserialize)]
struct Auth {
    token: String,
}

fn get_auth_token(image: &str) -> Result<String, Error> {
    let query = format!("service=registry.docker.io&scope=repository:library/{image}:pull");
    let url = format!("https://auth.docker.io/token?{}", query);
    let res = blocking::get(url)?.json::<Auth>()?;

    Ok(res.token)
}

#[derive(Deserialize)]
struct Layer {
    digest: String,
}
#[derive(Deserialize)]
struct Manifest {
    layers: Vec<Layer>,
}

fn get_manifest(image_name: &str, token: &String) -> Result<Manifest, Error> {
    let url = format!("https://registry.hub.docker.com/v2/library/{image_name}/manifests/latest");

    let client = reqwest::blocking::Client::new();
    let manifest = client
        .get(url)
        .header(
            reqwest::header::AUTHORIZATION,
            "Bearer ".to_owned() + &token,
        )
        .header(
            reqwest::header::ACCEPT,
            "application/vnd.docker.distribution.manifest.v2+json",
        )
        .send()?
        .json::<Manifest>()?;

    Ok(manifest)
}

fn get_layer(image_name: &str, layer: &Layer, token: &String) -> Result<File, Error> {
    let url = format!(
        "https://registry.hub.docker.com/v2/library/{image_name}/blobs/{}",
        layer.digest
    );

    let client = reqwest::blocking::Client::new();
    let res = client
        .get(url)
        .header(
            reqwest::header::AUTHORIZATION,
            "Bearer ".to_owned() + &token,
        )
        .send()?;

    let mut bytes = Cursor::new(res.bytes()?);
    let mut file = tempfile::tempfile()?;
    std::io::copy(&mut bytes, &mut file)?;

    Ok(file)
}

pub fn fetch_image(image: &String, temp_path: &str) -> Result<(), Error> {
    let token = get_auth_token(image)?;
    let manifest = get_manifest(image, &token)?;

    for layer in manifest.layers.iter() {
        let mut file = get_layer(image, layer, &token)?;
        file.seek(SeekFrom::Start(0))?;
        let decoded = GzDecoder::new(file);
        Archive::new(decoded).unpack(temp_path)?;
    }

    Ok(())
}
