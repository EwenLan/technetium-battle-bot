use std::env;

use technetium_battle_bot::transport::serve;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let port = env::args()
        .nth(technetium_battle_bot::rules::constants::CMD_PORT_ARG_INDEX)
        .ok_or("missing port argument")?;
    let port: u16 = port.parse()?;
    serve(port)?;
    Ok(())
}
