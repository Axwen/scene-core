fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (json, exit_code) = match scene_core_engine::run(&args) {
        Ok(output) => (output.json, output.exit_code),
        Err(error) => {
            let exit_code = error.code.exit_code();
            let json = serde_json::to_string(&error).expect("CLI error serializes");
            (json, exit_code)
        }
    };
    if !json.is_empty() {
        println!("{json}");
    }
    std::process::exit(i32::from(exit_code));
}
