use std::io::{self, BufRead, BufReader, Cursor};
use std::process::{Child, ChildStdout, Command, Stdio};
use xml::reader::{EventReader, XmlEvent};

#[derive(Clone, Debug, PartialEq)]
pub struct GpuProcessActivity {
    pub pid: i32,
    pub name: String,
    pub gpu_time_ms_per_s: f64,
}

pub struct PowermetricsSampler {
    child: Child,
    output: BufReader<ChildStdout>,
}

impl PowermetricsSampler {
    pub fn start() -> io::Result<Self> {
        let mut command = if unsafe { libc::geteuid() } == 0 {
            Command::new("/usr/bin/powermetrics")
        } else {
            let mut command = Command::new("/usr/bin/sudo");
            command.args(["-n", "/usr/bin/powermetrics"]);
            command
        };
        let mut child = command
            .args([
                "--sample-rate",
                "1000",
                "--sample-count",
                "-1",
                "--buffer-size",
                "1",
                "--format",
                "plist",
                "--samplers",
                "tasks",
                "--show-process-gpu",
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;
        let output = child
            .stdout
            .take()
            .ok_or_else(|| io::Error::other("powermetrics stdout is unavailable"))?;
        Ok(Self {
            child,
            output: BufReader::new(output),
        })
    }

    pub fn next_sample(&mut self) -> io::Result<Vec<GpuProcessActivity>> {
        let mut data = Vec::new();
        let read = self.output.read_until(0, &mut data)?;
        if read == 0 {
            let status = self.child.try_wait()?.map_or_else(
                || "without an exit status".to_owned(),
                |status| status.to_string(),
            );
            return Err(io::Error::other(format!("powermetrics stopped {status}")));
        }
        if data.last() == Some(&0) {
            data.pop();
        }
        parse_sample(&data)
    }
}

impl Drop for PowermetricsSampler {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[derive(Default)]
struct ActivityBuilder {
    pid: Option<i32>,
    name: String,
    gpu_time_ms_per_s: Option<f64>,
}

pub fn parse_sample(data: &[u8]) -> io::Result<Vec<GpuProcessActivity>> {
    let parser = EventReader::new(Cursor::new(data));
    let mut activities = Vec::new();
    let mut pending_key = String::new();
    let mut captured_tag = String::new();
    let mut captured_text = String::new();
    let mut array_depth = 0usize;
    let mut dictionary_depth = 0usize;
    let mut tasks_array_depth = None;
    let mut task_dictionary_depth = None;
    let mut task = None::<ActivityBuilder>;

    for event in parser {
        match event.map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))? {
            XmlEvent::StartElement { name, .. } => match name.local_name.as_str() {
                "array" => {
                    array_depth += 1;
                    if pending_key == "tasks" && tasks_array_depth.is_none() {
                        tasks_array_depth = Some(array_depth);
                    }
                    pending_key.clear();
                }
                "dict" => {
                    dictionary_depth += 1;
                    if tasks_array_depth.is_some()
                        && task_dictionary_depth.is_none()
                        && array_depth == tasks_array_depth.unwrap_or_default()
                    {
                        task_dictionary_depth = Some(dictionary_depth);
                        task = Some(ActivityBuilder::default());
                    }
                }
                "key" | "integer" | "real" | "string" => {
                    captured_tag = name.local_name;
                    captured_text.clear();
                }
                _ => {}
            },
            XmlEvent::Characters(text) | XmlEvent::CData(text) => {
                if !captured_tag.is_empty() {
                    captured_text.push_str(&text);
                }
            }
            XmlEvent::EndElement { name } => match name.local_name.as_str() {
                "key" => {
                    pending_key = captured_text.trim().to_owned();
                    captured_tag.clear();
                    captured_text.clear();
                }
                "integer" | "real" | "string" => {
                    if let Some(task) = task.as_mut() {
                        match pending_key.as_str() {
                            "pid" => task.pid = captured_text.trim().parse().ok(),
                            "name" => task.name = captured_text.to_owned(),
                            "gputime_ms_per_s" => {
                                task.gpu_time_ms_per_s = captured_text.trim().parse().ok()
                            }
                            _ => {}
                        }
                    }
                    pending_key.clear();
                    captured_tag.clear();
                    captured_text.clear();
                }
                "dict" => {
                    if task_dictionary_depth == Some(dictionary_depth) {
                        if let Some(task) = task.take() {
                            if let (Some(pid), Some(gpu_time_ms_per_s)) =
                                (task.pid, task.gpu_time_ms_per_s)
                            {
                                if pid > 0
                                    && gpu_time_ms_per_s.is_finite()
                                    && gpu_time_ms_per_s > 0.0
                                {
                                    activities.push(GpuProcessActivity {
                                        pid,
                                        name: task.name,
                                        gpu_time_ms_per_s,
                                    });
                                }
                            }
                        }
                        task_dictionary_depth = None;
                    }
                    dictionary_depth = dictionary_depth.saturating_sub(1);
                }
                "array" => {
                    if tasks_array_depth == Some(array_depth) {
                        tasks_array_depth = None;
                    }
                    array_depth = array_depth.saturating_sub(1);
                }
                _ => {}
            },
            _ => {}
        }
    }
    Ok(activities)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_only_active_gpu_tasks() {
        let input = br#"<?xml version="1.0" encoding="UTF-8"?>
<plist version="1.0"><dict>
<key>tasks</key><array>
<dict><key>pid</key><integer>42</integer><key>name</key><string>Metal App</string><key>gputime_ms_per_s</key><real>127.5</real></dict>
<dict><key>pid</key><integer>43</integer><key>name</key><string>Idle</string><key>gputime_ms_per_s</key><real>0</real></dict>
</array>
<key>all_tasks</key><dict><key>gputime_ms_per_s</key><real>127.5</real></dict>
</dict></plist>"#;
        assert_eq!(
            parse_sample(input).unwrap(),
            vec![GpuProcessActivity {
                pid: 42,
                name: "Metal App".into(),
                gpu_time_ms_per_s: 127.5,
            }]
        );
    }

    #[test]
    fn rejects_malformed_plist() {
        assert!(parse_sample(b"<plist><dict>").is_err());
    }
}
