use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

pub struct MatchedProcess {
    pub pid: i32,
    pub name: String,
    pub cmd: Vec<String>,
    pub rss_bytes: u64,
}

pub trait ProcessEnumerator: Send {
    fn full_rescan(&mut self, patterns: &[String]) -> Vec<MatchedProcess>;
    fn refresh_rss(&mut self) -> Vec<MatchedProcess>;
}

const LLM_NAMES: &[&str] = &[
    "ollama",
    "ollama_llama_server",
    "mlx_lm",
    "llama-server",
    "llama-cli",
    "llamafile",
    "lmstudio",
    "LM Studio",
    "python",
    "Python",
    "omlx",
    "vllm",
    "exo",
];

pub struct SysinfoScanner {
    sys: System,
    last_pids: Vec<i32>,
}

impl SysinfoScanner {
    pub fn new() -> Self {
        Self {
            sys: System::new(),
            last_pids: Vec::new(),
        }
    }

    fn matches_patterns(proc_name: &str, cmd: &[String], patterns: &[String]) -> bool {
        let names_to_check: &[&str] = if patterns.is_empty() {
            LLM_NAMES
        } else {
            return patterns.iter().any(|p| {
                proc_name.contains(p.as_str())
                    || cmd.iter().any(|c| c.contains(p.as_str()))
            });
        };

        let name_lower = proc_name.to_lowercase();
        if names_to_check.iter().any(|n| name_lower.contains(&n.to_lowercase())) {
            return true;
        }
        cmd.iter().any(|c| {
            let cl = c.to_lowercase();
            names_to_check.iter().any(|n| cl.contains(&n.to_lowercase()))
        })
    }
}

impl ProcessEnumerator for SysinfoScanner {
    fn full_rescan(&mut self, patterns: &[String]) -> Vec<MatchedProcess> {
        self.sys.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing()
                .with_memory()
                .with_cmd(UpdateKind::Always),
        );

        let mut matched = Vec::new();
        for (pid, proc_info) in self.sys.processes() {
            let name = proc_info.name().to_string_lossy().to_string();
            let cmd: Vec<String> = proc_info.cmd().iter().map(|s| s.to_string_lossy().to_string()).collect();

            if Self::matches_patterns(&name, &cmd, patterns) {
                matched.push(MatchedProcess {
                    pid: pid.as_u32() as i32,
                    name,
                    cmd,
                    rss_bytes: proc_info.memory(),
                });
            }
        }

        self.last_pids = matched.iter().map(|m| m.pid).collect();
        matched
    }

    fn refresh_rss(&mut self) -> Vec<MatchedProcess> {
        let pids: Vec<sysinfo::Pid> = self.last_pids.iter().map(|&p| sysinfo::Pid::from_u32(p as u32)).collect();
        self.sys.refresh_processes_specifics(
            ProcessesToUpdate::Some(&pids),
            true,
            ProcessRefreshKind::nothing().with_memory(),
        );

        let mut matched = Vec::new();
        for &pid in &self.last_pids {
            let spid = sysinfo::Pid::from_u32(pid as u32);
            if let Some(proc_info) = self.sys.process(spid) {
                matched.push(MatchedProcess {
                    pid,
                    name: proc_info.name().to_string_lossy().to_string(),
                    cmd: proc_info.cmd().iter().map(|s| s.to_string_lossy().to_string()).collect(),
                    rss_bytes: proc_info.memory(),
                });
            }
        }
        matched
    }
}
