use super::*;

pub(super) fn get_kde_display_layout() -> Result<DisplayLayout, String> {
    let value = compositor::kscreen_json()?;
    parse_kde_display_layout(&value)
}

fn parse_kde_display_layout(value: &serde_json::Value) -> Result<DisplayLayout, String> {
    let outputs = compositor::kde_outputs_from_value(value)?;

    fn parse_kde_mode(value: &serde_json::Value) -> Option<DisplayMode> {
        let width = value
            .get("size")
            .and_then(|size| size.get("width"))
            .and_then(|v| v.as_u64())
            .or_else(|| value.get("width").and_then(|v| v.as_u64()))? as u32;
        let height = value
            .get("size")
            .and_then(|size| size.get("height"))
            .and_then(|v| v.as_u64())
            .or_else(|| value.get("height").and_then(|v| v.as_u64()))? as u32;
        let refresh_rate = value
            .get("refreshRate")
            .and_then(|v| v.as_f64())
            .map(|rate| if rate > 1000.0 { rate / 1000.0 } else { rate })?;
        let backend_id = value.get("id").and_then(|id| {
            id.as_str()
                .map(str::to_string)
                .or_else(|| id.as_u64().map(|id| id.to_string()))
        });
        Some(make_display_mode_with_backend_id(
            width,
            height,
            refresh_rate,
            backend_id,
        ))
    }

    let mut displays = Vec::new();
    for output in outputs {
        if !output
            .get("enabled")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            continue;
        }

        let connector = output
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing KDE output name".to_string())?;
        let size = output
            .get("size")
            .and_then(|v| v.as_object())
            .ok_or_else(|| format!("Missing KDE size for {connector}"))?;
        let pos = output
            .get("pos")
            .and_then(|v| v.as_object())
            .ok_or_else(|| format!("Missing KDE position for {connector}"))?;
        let transform = match output.get("rotation") {
            Some(value) if value.is_number() => match value.as_u64().unwrap_or(1) {
                2 => 270,
                4 => 180,
                8 => 90,
                _ => 0,
            },
            Some(value) => match value.as_str().unwrap_or("none") {
                "right" => 90,
                "left" => 270,
                "inverted" => 180,
                _ => 0,
            },
            None => 0,
        };
        let scale = output.get("scale").and_then(|v| v.as_f64()).unwrap_or(1.0);
        let mode_value = output
            .get("currentMode")
            .filter(|mode| mode.is_object())
            .or_else(|| {
                let id = output.get("currentModeId")?;
                let modes = output.get("modes")?.as_array()?;
                modes.iter().find(|mode| mode.get("id") == Some(id))
            });
        let current_mode = mode_value.and_then(parse_kde_mode).unwrap_or_else(|| {
            make_display_mode(
                size.get("width").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
                size.get("height").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
                output
                    .get("currentMode")
                    .and_then(|mode| mode.get("refreshRate"))
                    .and_then(|v| v.as_f64())
                    .map(|rate| if rate > 1000.0 { rate / 1000.0 } else { rate })
                    .unwrap_or(60.0),
            )
        });
        let available_modes = dedupe_modes(
            output
                .get("modes")
                .and_then(|v| v.as_array())
                .map(|modes| modes.iter().filter_map(parse_kde_mode).collect())
                .unwrap_or_else(|| vec![current_mode.clone()]),
        );

        displays.push(DisplayInfo {
            connector: connector.to_string(),
            width: current_mode.width,
            height: current_mode.height,
            refresh_rate: current_mode.refresh_rate,
            scale,
            x: pos.get("x").and_then(|v| v.as_i64()).unwrap_or(0) as i32,
            y: pos.get("y").and_then(|v| v.as_i64()).unwrap_or(0) as i32,
            transform,
            primary: output.get("priority").and_then(|v| v.as_i64()).unwrap_or(0) == 1,
            current_mode,
            available_modes,
            refresh_policy: RefreshPolicy::Fixed,
            supports_dynamic_refresh: false,
        });
    }

    Ok(DisplayLayout { displays })
}

pub(super) fn apply_kde_display_layout(layout: &DisplayLayout) -> Result<(), String> {
    if layout.displays.is_empty() {
        return Err("No displays in layout".into());
    }

    let mut args: Vec<String> = Vec::new();
    let value = compositor::kscreen_json()?;
    let outputs = compositor::kde_outputs_from_value(&value)?;
    let available_outputs = compositor::kde_output_names_from_value(&value)?;
    for connector in omitted_output_names(layout, &available_outputs) {
        args.push(format!("output.{connector}.disable"));
    }

    for display in &layout.displays {
        if display.refresh_policy == RefreshPolicy::Dynamic {
            return Err(format!(
                "Dynamic refresh is not supported for {} on KDE",
                display.connector
            ));
        }

        args.push(format!("output.{}.enable", display.connector));
        args.push(format!(
            "output.{}.mode.{}",
            display.connector,
            kde_mode_argument(
                display,
                outputs
                    .iter()
                    .find(|output| output.get("name").and_then(|v| v.as_str())
                        == Some(display.connector.as_str()))
            )
        ));
        args.push(format!(
            "output.{}.position.{},{}",
            display.connector, display.x, display.y
        ));
        args.push(format!(
            "output.{}.rotation.{}",
            display.connector,
            match display.transform {
                90 => "right",
                180 => "inverted",
                270 => "left",
                _ => "none",
            }
        ));
        args.push(format!(
            "output.{}.scale.{:.6}",
            display.connector,
            display.scale.max(0.1)
        ));
        if display.primary {
            args.push(format!("output.{}.priority.1", display.connector));
        }
    }

    run_command("kscreen-doctor", &args)
}

fn kde_mode_argument(display: &DisplayInfo, output: Option<&serde_json::Value>) -> String {
    if let Some(modes) = output
        .and_then(|output| output.get("modes"))
        .and_then(|modes| modes.as_array())
    {
        for mode in modes {
            let width = mode
                .get("size")
                .and_then(|size| size.get("width"))
                .and_then(|v| v.as_u64());
            let height = mode
                .get("size")
                .and_then(|size| size.get("height"))
                .and_then(|v| v.as_u64());
            let rate = mode
                .get("refreshRate")
                .and_then(|v| v.as_f64())
                .map(|rate| if rate > 1000.0 { rate / 1000.0 } else { rate });
            if width == Some(display.current_mode.width as u64)
                && height == Some(display.current_mode.height as u64)
                && rate.is_some_and(|rate| (rate - display.current_mode.refresh_rate).abs() < 0.01)
            {
                if let Some(id) = mode.get("id").and_then(|id| {
                    id.as_str()
                        .map(str::to_string)
                        .or_else(|| id.as_u64().map(|id| id.to_string()))
                }) {
                    return id;
                }
            }
        }
    }
    display.current_mode.mode_id.clone()
}

fn kde_output_logical_size(name: &str) -> Result<(i64, i64), String> {
    compositor::kde_output_logical_size_from_value(&compositor::kscreen_json()?, name)
}

fn kde_enabled_output_count() -> Result<usize, String> {
    compositor::kde_enabled_output_count_from_value(&compositor::kscreen_json()?)
}

fn kde_rotation_token(orientation: &Orientation) -> &'static str {
    match orientation {
        Orientation::Left => "right",
        Orientation::Right => "left",
        Orientation::Inverted => "inverted",
        Orientation::Normal => "none",
    }
}

fn rotated_secondary_position(
    orientation: &Orientation,
    primary_size: Result<(i64, i64), String>,
) -> Result<(i64, i64), String> {
    let (width, height) = primary_size?;
    let (rot_w, rot_h) = match orientation {
        Orientation::Left | Orientation::Right => (height, width),
        Orientation::Normal | Orientation::Inverted => (width, height),
    };
    Ok(match orientation {
        Orientation::Left => (-rot_w, 0),
        Orientation::Right => (rot_w, 0),
        Orientation::Inverted => (0, -rot_h),
        Orientation::Normal => (0, rot_h),
    })
}

pub(super) fn set_kde_orientation(orientation: &Orientation) -> Result<(), String> {
    let token = kde_rotation_token(orientation);
    let enabled_count = kde_enabled_output_count().unwrap_or(1);

    if enabled_count <= 1 {
        let args = vec![
            format!("output.{PRIMARY_INTERNAL_CONNECTOR}.enable"),
            format!("output.{PRIMARY_INTERNAL_CONNECTOR}.position.0,0"),
            format!("output.{PRIMARY_INTERNAL_CONNECTOR}.rotation.{token}"),
        ];
        return run_command("kscreen-doctor", &args);
    }

    let (pos_x, pos_y) = rotated_secondary_position(
        orientation,
        kde_output_logical_size(PRIMARY_INTERNAL_CONNECTOR),
    )?;

    let args = vec![
        format!("output.{PRIMARY_INTERNAL_CONNECTOR}.enable"),
        format!("output.{SECONDARY_INTERNAL_CONNECTOR}.enable"),
        format!("output.{PRIMARY_INTERNAL_CONNECTOR}.rotation.{token}"),
        format!("output.{SECONDARY_INTERNAL_CONNECTOR}.rotation.{token}"),
        format!("output.{PRIMARY_INTERNAL_CONNECTOR}.position.0,0"),
        format!("output.{SECONDARY_INTERNAL_CONNECTOR}.position.{pos_x},{pos_y}"),
    ];

    run_command("kscreen-doctor", &args)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_kscreen_hertz_current_mode_id_and_numeric_rotation() {
        let value = serde_json::json!({"outputs": [{
            "name": "eDP-1", "enabled": true,
            "size": {"width": 2880, "height": 1800},
            "pos": {"x": 0, "y": 0}, "rotation": 2,
            "scale": 1.6, "priority": 1, "currentModeId": "27",
            "modes": [
                {"id": "27", "size": {"width": 2880, "height": 1800}, "refreshRate": 120.0},
                {"id": "28", "size": {"width": 2880, "height": 1800}, "refreshRate": 60.001}
            ]
        }]});
        let layout = parse_kde_display_layout(&value).expect("current KScreen layout");
        let display = &layout.displays[0];
        assert_eq!(display.refresh_rate, 120.0);
        assert_eq!(display.current_mode.mode_id, "2880x1800@120");
        assert_eq!(display.current_mode.backend_mode_id.as_deref(), Some("27"));
        assert_eq!(display.transform, 270);
        assert_eq!(display.available_modes[1].refresh_rate, 60.001);
        let bottom = serde_json::json!({"name":"eDP-2","modes":[
            {"id":"1","size":{"width":2880,"height":1800},"refreshRate":120.0}
        ]});
        // A cloned top-panel mode ID must be resolved against the bottom panel.
        assert_eq!(kde_mode_argument(display, Some(&bottom)), "1");
    }

    #[test]
    fn preserves_legacy_millihertz_current_mode_and_string_rotation() {
        let value = serde_json::json!({"outputs": [{
            "name": "eDP-1", "enabled": true,
            "size": {"width": 2880, "height": 1800},
            "pos": {"x": 0, "y": 0}, "rotation": "left",
            "currentMode": {"width": 2880, "height": 1800, "refreshRate": 120000.0}
        }]});
        let layout = parse_kde_display_layout(&value).expect("legacy KScreen layout");
        assert_eq!(layout.displays[0].refresh_rate, 120.0);
        assert_eq!(layout.displays[0].transform, 270);
    }

    #[test]
    fn missing_primary_geometry_is_an_error_for_secondary_positioning() {
        let result =
            rotated_secondary_position(&Orientation::Normal, Err("missing geometry".into()));

        assert_eq!(
            result.expect_err("missing geometry should fail"),
            "missing geometry"
        );
    }

    #[test]
    fn secondary_position_uses_rotated_primary_geometry() {
        assert_eq!(
            rotated_secondary_position(&Orientation::Left, Ok((1200, 800))).expect("position"),
            (-800, 0)
        );
        assert_eq!(
            rotated_secondary_position(&Orientation::Normal, Ok((1200, 800))).expect("position"),
            (0, 800)
        );
    }
}
