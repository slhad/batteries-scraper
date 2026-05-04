use std::env;
use std::fmt::Write as _;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::Instant;
use std::time::{SystemTime, UNIX_EPOCH};
use tiny_http::{Header, Method, Response, Server, StatusCode};

const DEFAULT_BIND: &str = "192.168.0.118:12321";
const DEFAULT_DEVICE_PATH: &str = "/org/freedesktop/UPower/devices/battery_hidpp_battery_0";
const DEFAULT_ROUTE: &str = "/device/dev00000000";

fn main() -> std::io::Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.iter().any(|arg| arg == "--install-systemd-user") {
        install_systemd_user_service()?;
        return Ok(());
    }

    let bind = env::var("BIND_ADDR").unwrap_or_else(|_| DEFAULT_BIND.to_string());
    let device_path = env::var("UPOWER_DEVICE").unwrap_or_else(|_| DEFAULT_DEVICE_PATH.to_string());

    let server = Server::http(&bind).map_err(std::io::Error::other)?;
    eprintln!("listening on http://{bind}{DEFAULT_ROUTE}");

    for request in server.incoming_requests() {
        let started = Instant::now();
        let method = request.method().clone();
        let path = request.url().to_string();
        eprintln!("request: {} {}", method.as_str(), path);

        let response = match (&method, path.as_str()) {
            (&Method::Get, "/") | (&Method::Get, DEFAULT_ROUTE) => {
                match read_battery_percentage(&device_path) {
                    Ok(percentage) => ok_response(path.as_str(), percentage),
                    Err(error) => error_response(503, &error),
                }
            }
            (&Method::Head, "/") | (&Method::Head, DEFAULT_ROUTE) => {
                match read_battery_percentage(&device_path) {
                    Ok(_) => text_response(200, "text/html; charset=utf-8", String::new()),
                    Err(error) => error_response(503, &error),
                }
            }
            (&Method::Get, "/health") => {
                let status = if read_battery_percentage(&device_path).is_ok() {
                    "ok"
                } else {
                    "degraded"
                };
                text_response(
                    200,
                    "application/json; charset=utf-8",
                    format!("{{\"status\":\"{status}\"}}"),
                )
            }
            _ => text_response(404, "text/plain; charset=utf-8", "not found".to_string()),
        };

        if let Err(error) = request.respond(response) {
            eprintln!("request failed: {error}");
        } else {
            eprintln!("response sent in {} ms", started.elapsed().as_millis());
        }
    }

    Ok(())
}

fn install_systemd_user_service() -> std::io::Result<()> {
    let exe = env::current_exe()?;
    let service_dir = systemd_user_dir()?;
    fs::create_dir_all(&service_dir)?;

    let service_path = service_dir.join("batteries-scraper.service");
    let service = build_systemd_user_service(
        exe.as_os_str().to_string_lossy().as_ref(),
        DEFAULT_BIND,
        DEFAULT_DEVICE_PATH,
    );

    fs::write(&service_path, service)?;

    run_systemctl_user(&["daemon-reload"])?;
    run_systemctl_user(&["enable", "--now", "batteries-scraper.service"])?;

    println!("installed {}", service_path.display());
    Ok(())
}

fn systemd_user_dir() -> std::io::Result<PathBuf> {
    let home = env::var("HOME")
        .map(PathBuf::from)
        .map_err(|error| std::io::Error::other(format!("HOME not set: {error}")))?;
    Ok(home.join(".config/systemd/user"))
}

fn run_systemctl_user(args: &[&str]) -> std::io::Result<()> {
    let status = Command::new("systemctl")
        .arg("--user")
        .args(args)
        .status()?;

    if status.success() {
        Ok(())
    } else {
        Err(std::io::Error::other(format!(
            "systemctl --user {} failed with status {status}",
            args.join(" ")
        )))
    }
}

fn shell_escape(input: &str) -> String {
    if input.bytes().all(
        |byte| matches!(byte, b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'/' | b'.' | b'-' | b'_'),
    ) {
        input.to_string()
    } else {
        format!("'{}'", input.replace('\'', "'\\''"))
    }
}

fn read_battery_percentage(device_path: &str) -> Result<u8, String> {
    let output = Command::new("upower")
        .args(["-i", device_path])
        .output()
        .map_err(|error| format!("failed to run upower: {error}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("upower failed: {}", stderr.trim()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_battery_percentage(&stdout)
}

fn ok_response(path: &str, percentage: u8) -> Response<std::io::Cursor<Vec<u8>>> {
    if path == DEFAULT_ROUTE {
        let last_update = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_secs().to_string())
            .unwrap_or_else(|_| "0".to_string());
        let body = build_device_xml(percentage, &last_update);
        text_response(200, "text/xml; charset=utf-8", body)
    } else {
        text_response(200, "text/plain; charset=utf-8", percentage.to_string())
    }
}

fn error_response(status: u16, message: &str) -> Response<std::io::Cursor<Vec<u8>>> {
    text_response(
        status,
        "application/json; charset=utf-8",
        format!("{{\"error\":\"{message}\"}}"),
    )
}

fn text_response(
    status: u16,
    content_type: &str,
    body: String,
) -> Response<std::io::Cursor<Vec<u8>>> {
    let mut response = Response::from_string(body).with_status_code(StatusCode(status));
    if let Ok(header) = Header::from_bytes("Content-Type", content_type) {
        response = response.with_header(header);
    }
    response
}

fn parse_battery_percentage(stdout: &str) -> Result<u8, String> {
    for line in stdout.lines() {
        let trimmed = line.trim();
        if let Some(value) = trimmed.strip_prefix("percentage:") {
            let digits: String = value.chars().filter(|ch| ch.is_ascii_digit()).collect();
            return digits
                .parse::<u8>()
                .map_err(|error| format!("invalid battery percentage: {error}"));
        }
    }

    Err("battery percentage not found in upower output".to_string())
}

fn build_device_xml(percentage: u8, last_update: &str) -> String {
    let mut body = String::new();
    let _ = write!(
        body,
        concat!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>",
            "<xml>",
            "<device_id>dev00000000</device_id>",
            "<device_name>G502 LIGHTSPEED Wireless Gaming Mouse</device_name>",
            "<device_type>Mouse</device_type>",
            "<battery_percent>{0:.2}</battery_percent>",
            "<battery_voltage>0.00</battery_voltage>",
            "<mileage>0.00</mileage>",
            "<charging>false</charging>",
            "<last_update>{1}</last_update>",
            "</xml>"
        ),
        percentage as f32, last_update
    );
    body
}

fn build_systemd_user_service(exe: &str, bind: &str, device_path: &str) -> String {
    format!(
        concat!(
            "[Unit]\n",
            "Description=Battery scraper HTTP bridge\n",
            "After=network-online.target\n",
            "Wants=network-online.target\n\n",
            "[Service]\n",
            "Type=simple\n",
            "ExecStart={}\n",
            "Restart=always\n",
            "RestartSec=3\n",
            "Environment=BIND_ADDR={}\n",
            "Environment=UPOWER_DEVICE={}\n\n",
            "[Install]\n",
            "WantedBy=default.target\n"
        ),
        shell_escape(exe),
        bind,
        device_path
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_battery_percentage_accepts_normal_upower_output() {
        let stdout = "\
  native-path:          hidpp_battery_0
  model:                G502 LIGHTSPEED Wireless Gaming Mouse
  percentage:          85%
";
        assert_eq!(parse_battery_percentage(stdout).unwrap(), 85);
    }

    #[test]
    fn parse_battery_percentage_ignores_surrounding_whitespace() {
        let stdout = "percentage:             7%\n";
        assert_eq!(parse_battery_percentage(stdout).unwrap(), 7);
    }

    #[test]
    fn parse_battery_percentage_rejects_missing_percentage() {
        let stdout = "state: discharging\n";
        let error = parse_battery_percentage(stdout).unwrap_err();
        assert!(error.contains("battery percentage not found"));
    }

    #[test]
    fn parse_battery_percentage_rejects_non_numeric_values() {
        let stdout = "percentage: unknown%\n";
        let error = parse_battery_percentage(stdout).unwrap_err();
        assert!(error.contains("invalid battery percentage"));
    }

    #[test]
    fn build_device_xml_matches_windows_shape() {
        let xml = build_device_xml(85, "1777936068");
        assert_eq!(
            xml,
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\
<xml>\
<device_id>dev00000000</device_id>\
<device_name>G502 LIGHTSPEED Wireless Gaming Mouse</device_name>\
<device_type>Mouse</device_type>\
<battery_percent>85.00</battery_percent>\
<battery_voltage>0.00</battery_voltage>\
<mileage>0.00</mileage>\
<charging>false</charging>\
<last_update>1777936068</last_update>\
</xml>"
        );
    }

    #[test]
    fn build_device_xml_formats_fractional_percentage_as_two_decimals() {
        let xml = build_device_xml(7, "1");
        assert!(xml.contains("<battery_percent>7.00</battery_percent>"));
    }

    #[test]
    fn build_systemd_user_service_contains_expected_fields() {
        let unit = build_systemd_user_service(
            "/tmp/batteries-scraper",
            "192.168.0.118:12321",
            "/org/freedesktop/UPower/devices/battery_hidpp_battery_0",
        );

        assert!(unit.contains("Description=Battery scraper HTTP bridge"));
        assert!(unit.contains("ExecStart=/tmp/batteries-scraper"));
        assert!(unit.contains("Environment=BIND_ADDR=192.168.0.118:12321"));
        assert!(unit.contains(
            "Environment=UPOWER_DEVICE=/org/freedesktop/UPower/devices/battery_hidpp_battery_0"
        ));
        assert!(unit.contains("WantedBy=default.target"));
    }

    #[test]
    fn build_systemd_user_service_escapes_exec_path() {
        let unit = build_systemd_user_service(
            "/tmp/batteries scraper/bin",
            DEFAULT_BIND,
            DEFAULT_DEVICE_PATH,
        );
        assert!(unit.contains("ExecStart='/tmp/batteries scraper/bin'"));
    }

    #[test]
    fn shell_escape_leaves_simple_paths_untouched() {
        assert_eq!(
            shell_escape("/tmp/batteries-scraper_1.0"),
            "/tmp/batteries-scraper_1.0"
        );
    }

    #[test]
    fn shell_escape_quotes_paths_with_spaces_and_single_quotes() {
        assert_eq!(
            shell_escape("/tmp/it's battery scraper"),
            "'/tmp/it'\\''s battery scraper'"
        );
    }

    #[test]
    fn ok_response_returns_plain_text_for_root_path() {
        let response = ok_response("/", 42);
        let header = response
            .headers()
            .iter()
            .find(|header| header.field.equiv("Content-Type"))
            .unwrap();
        assert_eq!(header.value.as_str(), "text/plain; charset=utf-8");
    }

    #[test]
    fn ok_response_returns_xml_for_device_path() {
        let response = ok_response(DEFAULT_ROUTE, 42);
        let header = response
            .headers()
            .iter()
            .find(|header| header.field.equiv("Content-Type"))
            .unwrap();
        assert_eq!(header.value.as_str(), "text/xml; charset=utf-8");
    }

    #[test]
    fn error_response_uses_json_content_type() {
        let response = error_response(503, "boom");
        let header = response
            .headers()
            .iter()
            .find(|header| header.field.equiv("Content-Type"))
            .unwrap();
        assert_eq!(header.value.as_str(), "application/json; charset=utf-8");
    }
}
