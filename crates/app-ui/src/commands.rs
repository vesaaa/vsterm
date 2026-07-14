//! Quick commands stored in ~/.vsterm/commands.yaml

use serde::{Deserialize, Serialize};
use session_tree::AppPaths;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuickCommand {
    pub name: String,
    pub command: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CommandBook {
    #[serde(default)]
    pub commands: Vec<QuickCommand>,
}

impl CommandBook {
    pub fn path(paths: &AppPaths) -> PathBuf {
        paths.root.join("commands.yaml")
    }

    pub fn load_or_seed(paths: &AppPaths) -> anyhow::Result<Self> {
        let path = Self::path(paths);
        if path.exists() {
            let text = fs::read_to_string(&path)?;
            return Ok(serde_yaml::from_str(&text)?);
        }
        let book = Self::default_seed();
        book.save(paths)?;
        Ok(book)
    }

    pub fn save(&self, paths: &AppPaths) -> anyhow::Result<()> {
        paths.ensure_dirs()?;
        let text = serde_yaml::to_string(self)?;
        fs::write(Self::path(paths), text)?;
        Ok(())
    }

    pub fn default_seed() -> Self {
        Self {
            commands: vec![
                QuickCommand {
                    name: "df -h".into(),
                    command: "df -h\n".into(),
                    description: Some("磁盘占用".into()),
                },
                QuickCommand {
                    name: "free -h".into(),
                    command: "free -h\n".into(),
                    description: Some("内存".into()),
                },
                QuickCommand {
                    name: "uptime".into(),
                    command: "uptime\n".into(),
                    description: Some("运行时间".into()),
                },
                QuickCommand {
                    name: "ip a".into(),
                    command: "ip a\n".into(),
                    description: Some("网卡地址".into()),
                },
                QuickCommand {
                    name: "top once".into(),
                    command: "top -b -n 1 | head -n 20\n".into(),
                    description: Some("进程快照".into()),
                },
                QuickCommand {
                    name: "Get-Process".into(),
                    command: "Get-Process | Sort-Object CPU -Descending | Select-Object -First 15\n".into(),
                    description: Some("Windows 进程 TOP".into()),
                },
            ],
        }
    }
}
