//! Lightweight i18n — English + Simplified Chinese (first batch).

use serde::{Deserialize, Serialize};
use std::sync::RwLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum Locale {
    #[default]
    ZhCn,
    En,
}

impl Locale {
    pub fn label(self) -> &'static str {
        match self {
            Self::ZhCn => "简体中文",
            Self::En => "English",
        }
    }

    pub fn code(self) -> &'static str {
        match self {
            Self::ZhCn => "zh-CN",
            Self::En => "en",
        }
    }

    pub fn from_code(code: &str) -> Self {
        match code {
            "en" | "en-US" | "en-GB" => Self::En,
            _ => Self::ZhCn,
        }
    }
}

static LOCALE: RwLock<Locale> = RwLock::new(Locale::ZhCn);

pub fn current() -> Locale {
    *LOCALE.read().unwrap_or_else(|e| e.into_inner())
}

pub fn set(locale: Locale) {
    if let Ok(mut g) = LOCALE.write() {
        *g = locale;
    }
}

/// Translate a message key for the active locale.
pub fn t(key: &str) -> String {
    tr(current(), key).to_string()
}

pub fn tr(locale: Locale, key: &str) -> &'static str {
    match locale {
        Locale::ZhCn => zh(key),
        Locale::En => en(key),
    }
}

fn zh(key: &str) -> &'static str {
    match key {
        "app.name" => "VsTerm",
        "menu.file" => "文件",
        "menu.file.refresh_tree" => "刷新会话树",
        "menu.file.new_local_shell" => "新建本地 Shell",
        "menu.file.exit" => "退出",
        "menu.connection" => "连接",
        "menu.connection.close" => "关闭当前连接",
        "menu.language" => "语言",
        "menu.view" => "视图",
        "tab.servers" => "服务器",
        "tab.monitor" => "系统监控",
        "tab.files" => "文件",
        "tab.commands" => "命令",
        "conn.title" => "连接",
        "conn.empty" => "无活动连接",
        "conn.hint" => "切换不影响后台接收",
        "conn.connected" => "已连接",
        "conn.connecting" => "连接中",
        "conn.disconnected" => "已断开",
        "conn.failed" => "失败",
        "tree.heading" => "会话",
        "tree.local_shell" => "＋ 本地 Shell",
        "tree.empty" => "暂无会话 — 首次启动会写入演示数据到 ~/.vsterm/",
        "tree.open_hint" => "双击打开连接",
        "monitor.cpu" => "CPU",
        "monitor.memory" => "内存",
        "monitor.swap" => "交换",
        "monitor.processes" => "进程列表",
        "monitor.pid" => "PID",
        "monitor.name" => "进程名",
        "monitor.cpu_pct" => "CPU%",
        "monitor.mem" => "内存",
        "monitor.network" => "网络流量",
        "monitor.interface" => "网卡",
        "monitor.rx" => "下行",
        "monitor.tx" => "上行",
        "monitor.storage" => "存储",
        "monitor.fs" => "挂载点",
        "monitor.size" => "容量",
        "monitor.capacity" => "已用/总大小",
        "monitor.used" => "已用",
        "monitor.avail" => "可用",
        "monitor.use_pct" => "使用率",
        "monitor.no_connection" => "请先打开一个终端连接",
        "monitor.refresh" => "刷新中…",
        "toolbar.sysinfo" => "系统信息",
        "toolbar.files" => "文件传输",
        "toolbar.commands" => "常用命令",
        "main.tab.terminal" => "终端",
        "main.tab.sysinfo" => "系统信息",
        "main.tab.routes" => "路由信息",
        "routes.title" => "路由表",
        "routes.refresh" => "刷新",
        "routes.destination" => "目标",
        "routes.gateway" => "网关",
        "routes.mask" => "掩码",
        "routes.flags" => "跃点/接口",
        "routes.iface" => "接口",
        "routes.raw" => "原始输出",
        "routes.empty" => "暂无路由信息",
        "sysinfo.title" => "系统信息",
        "sysinfo.os" => "操作系统",
        "sysinfo.kernel" => "内核",
        "sysinfo.kernel_ver" => "内核版本",
        "sysinfo.arch" => "架构",
        "sysinfo.hostname" => "主机名",
        "sysinfo.cpu_model" => "CPU 型号",
        "sysinfo.cpu_usage" => "CPU 占用",
        "sysinfo.nics" => "网络接口",
        "sysinfo.disks" => "磁盘存储",
        "sysinfo.close" => "关闭",
        "sysinfo.unavailable" => "暂无主机信息（无活动连接）",
        "bottom.files.hint" => "文件传输（SFTP）— 阶段内先提供界面骨架，远端传输随后接入",
        "bottom.files.local" => "本地",
        "bottom.files.remote" => "远端",
        "bottom.files.upload" => "上传 →",
        "bottom.files.download" => "← 下载",
        "bottom.commands.hint" => "点击命令会发送到当前终端",
        "bottom.commands.empty" => "暂无命令 — 可编辑 ~/.vsterm/commands.yaml",
        "bottom.commands.send" => "发送",
        "status.stage" => "阶段 1–3 · UI",
        "status.connections" => "连接",
        "status.opened_shell" => "已打开本地 Shell",
        "status.open_failed" => "打开失败",
        "status.closed" => "已关闭连接",
        "status.tree_reloaded" => "会话树已刷新",
        "status.lang_changed" => "语言已切换",
        "term.empty" => "无活动连接 — 从左侧打开会话或本地 Shell",
        _ => "???",
    }
}

fn en(key: &str) -> &'static str {
    match key {
        "app.name" => "VsTerm",
        "menu.file" => "File",
        "menu.file.refresh_tree" => "Refresh Session Tree",
        "menu.file.new_local_shell" => "New Local Shell",
        "menu.file.exit" => "Exit",
        "menu.connection" => "Connection",
        "menu.connection.close" => "Close Current",
        "menu.language" => "Language",
        "menu.view" => "View",
        "tab.servers" => "Servers",
        "tab.monitor" => "Monitor",
        "tab.files" => "Files",
        "tab.commands" => "Commands",
        "conn.title" => "Sessions",
        "conn.empty" => "No active connections",
        "conn.hint" => "Switching keeps background I/O",
        "conn.connected" => "Connected",
        "conn.connecting" => "Connecting",
        "conn.disconnected" => "Disconnected",
        "conn.failed" => "Failed",
        "tree.heading" => "Sessions",
        "tree.local_shell" => "+ Local Shell",
        "tree.empty" => "No sessions — demo data will be written to ~/.vsterm/",
        "tree.open_hint" => "Double-click to open",
        "monitor.cpu" => "CPU",
        "monitor.memory" => "Memory",
        "monitor.swap" => "Swap",
        "monitor.processes" => "Processes",
        "monitor.pid" => "PID",
        "monitor.name" => "Name",
        "monitor.cpu_pct" => "CPU%",
        "monitor.mem" => "Mem",
        "monitor.network" => "Network",
        "monitor.interface" => "Interface",
        "monitor.rx" => "RX",
        "monitor.tx" => "TX",
        "monitor.storage" => "Storage",
        "monitor.fs" => "Mount",
        "monitor.size" => "Size",
        "monitor.capacity" => "Used/Total",
        "monitor.used" => "Used",
        "monitor.avail" => "Avail",
        "monitor.use_pct" => "Use%",
        "monitor.no_connection" => "Open a terminal connection first",
        "monitor.refresh" => "Refreshing…",
        "toolbar.sysinfo" => "System Info",
        "toolbar.files" => "File Transfer",
        "toolbar.commands" => "Commands",
        "main.tab.terminal" => "Terminal",
        "main.tab.sysinfo" => "System Info",
        "main.tab.routes" => "Routes",
        "routes.title" => "Routing Table",
        "routes.refresh" => "Refresh",
        "routes.destination" => "Destination",
        "routes.gateway" => "Gateway",
        "routes.mask" => "Genmask",
        "routes.flags" => "Metric/If",
        "routes.iface" => "Interface",
        "routes.raw" => "Raw output",
        "routes.empty" => "No routes",
        "sysinfo.title" => "System Information",
        "sysinfo.os" => "OS",
        "sysinfo.kernel" => "Kernel",
        "sysinfo.kernel_ver" => "Kernel Version",
        "sysinfo.arch" => "Architecture",
        "sysinfo.hostname" => "Hostname",
        "sysinfo.cpu_model" => "CPU Model",
        "sysinfo.cpu_usage" => "CPU Usage",
        "sysinfo.nics" => "Network Interfaces",
        "sysinfo.disks" => "Disk Storage",
        "sysinfo.close" => "Close",
        "sysinfo.unavailable" => "No host info (no active connection)",
        "bottom.files.hint" => "File transfer (SFTP) — UI scaffold for now; remote transfer coming next",
        "bottom.files.local" => "Local",
        "bottom.files.remote" => "Remote",
        "bottom.files.upload" => "Upload →",
        "bottom.files.download" => "← Download",
        "bottom.commands.hint" => "Click a command to send it to the active terminal",
        "bottom.commands.empty" => "No commands — edit ~/.vsterm/commands.yaml",
        "bottom.commands.send" => "Send",
        "status.stage" => "Stage 1–3 · UI",
        "status.connections" => "Connections",
        "status.opened_shell" => "Local shell opened",
        "status.open_failed" => "Failed to open",
        "status.closed" => "Connection closed",
        "status.tree_reloaded" => "Session tree reloaded",
        "status.lang_changed" => "Language changed",
        "term.empty" => "No active connection — open a session or local shell on the left",
        _ => "???",
    }
}
