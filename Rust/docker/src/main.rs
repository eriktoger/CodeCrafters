mod fetch_image;
use std::fs::create_dir_all;

use anyhow::Result;

use crate::fetch_image::fetch_image;

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().collect();
    let image = &args[2];
    let command = &args[3];
    let command_args = &args[4..];

    //create a new directory and dev/null
    let new_dir = "temp";
    create_dir_all(format!("{new_dir}/dev/null"))?;

    fetch_image(image, &new_dir)?;

    // change to the new directory
    std::os::unix::fs::chroot(&new_dir)?;

    // Process isolation
    unsafe {
        libc::unshare(libc::CLONE_NEWPID);
    }

    let output = std::process::Command::new(command)
        .args(command_args)
        .output()?;

    let std_out = std::str::from_utf8(&output.stdout)?;
    print!("{}", std_out);

    let std_err = std::str::from_utf8(&output.stderr)?;
    eprint!("{}", std_err);

    match output.status.code() {
        Some(code) => std::process::exit(code),
        None => {}
    };

    Ok(())
}
