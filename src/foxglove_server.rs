pub fn create_foxglove_url(user: &str, url: &str, port: &str, layout_id: &str) -> String {
    // https://app.foxglove.dev/david-weis/view?ds=foxglove-websocket&ds.url=ws://127.0.0.1:8765/&layoutId=ea22e72c-f654-4743-925a-7143a510d390
    format!("https://app.foxglove.dev/{user}/view?ds=foxglove-websocket&ds.url=ws://{url}:{port}/&layoutId={layout_id}")
}
