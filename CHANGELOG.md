# Changelog

All notable releases are published as binaries on
[GitHub Releases](https://github.com/vesaaa/vsterm/releases).

Source development is private; tags that trigger CI live in this repository.
On each `v*` tag, CI also syncs `README.md`, `README.zh-CN.md`, `CHANGELOG.md`,
and `LICENSE` to the public `vesaaa/vsterm` main branch, and copies this file’s
section for that version into the GitHub Release notes.

## [1.5.0] — 2026-10-29

### Added
- **Update dialog with release notes**: when a new version is out, the dialog shows its version, release date and what's new (in your UI language), with **Download update**, **Open release page** and **Later**.
- **Background update check**, at most once a day, while you are idle (never during a login or file transfer). It stays silent unless there is a newer version; then the status bar shows "Update x.y.z". Switch it off with **Check for updates at startup** in the dialog.
- **Pro pets join the login**: when you log in with a Pro pet out, it throws its emblem at the login dialog (Laojun's taiji, the monkey's banana, the Buddha's 卍, a cloud from the Cloud Bun), with a short effect where it hits. With the Shatter effect, the glass cracks from that spot. The throw never delays the login.

### Changed
- **Shatter** connect effect: the real login dialog, with its fields, text and theme colours, now breaks into glass pieces that crack from the impact point and fly apart. Double-clicking a dropped session's status light plays it backwards.
- **Trail** connect effect: the login dialog shrinks into the session's status light (and grows back out of it on reconnect). The sparkle ring around a new login dialog is gone.
- The password login dialog keeps one size (450 × 280) in every state, and a changed host key is confirmed inside it instead of in a separate dialog.
- New icons for the Files pane's path sync buttons, so they no longer look like "go up a folder": a terminal over an up arrow (files → terminal) and a down arrow over a folder (terminal → files).
- The excavator pet is redrawn as one yellow machine; Enter now slaps the bucket down without squashing it.

### Fixed
- **False "host key changed" warnings** for servers that did not change: a host with several keys in `known_hosts` (Ed25519, ECDSA, RSA) is now checked against the key of the same type, and VsTerm prefers key types it already knows. When a key really changed, the dialog shows the new fingerprint and the command to check it on the server; **Trust and connect** replaces only that host's entry and reconnects with the login you entered, no `ssh-keygen -R` needed.

### 中文
- **新增**：更新对话框会显示新版本号、发布日期和更新内容（跟随界面语言），提供「下载更新」「打开发布页」「稍后」。
- **新增**：后台每天最多检查一次更新，只在空闲时进行（登录或传输文件时不检查）。没有新版本时不打扰；有新版本时，状态栏会显示「有新版本 x.y.z」。可在对话框里关闭「启动时自动检查更新」。
- **新增**：Pro 宠物参与登录：带着 Pro 宠物登录时，它会把自己的法宝扔向登录框（太上老君的太极、猴子的香蕉、金佛的卍、云团的一朵云），击中处有短暂特效；连接特效为「破碎」时，玻璃会从击中点裂开。扔东西不会拖慢登录。
- **改进**：「破碎」连接特效改为把真实的登录框（含输入框、文字和主题颜色）碎成玻璃片，从击中点裂开后飞散；双击已断开会话的状态灯会倒放这一过程。
- **改进**：「拖影」连接特效改为登录框缩小进会话的状态灯（重连时从状态灯放大出来），并去掉了登录框出现时的火花圈。
- **改进**：密码登录框在各种状态下大小固定为 450 × 280，主机密钥变更也直接在登录框里确认，不再弹出单独的对话框。
- **改进**：文件区的两个路径同步按钮换了新图标，不再像「返回上一级」：终端在上、上箭头在下表示文件区 → 终端；下箭头在上、文件夹在下表示终端 → 文件区。
- **改进**：暴躁挖掘机重新绘制为同一台黄色挖掘机，回车时铲斗拍下去，不再被压扁。
- **修复**：服务器没变却提示「主机密钥已变更」的误报：`known_hosts` 里同一主机有多种密钥（Ed25519、ECDSA、RSA）时，现在按相同类型比对，并优先使用已记录的密钥类型。密钥真的变了时，对话框显示新指纹和在服务器上核对的命令；点「信任并连接」只替换该主机的记录并用已输入的登录信息重新连接，无需再执行 `ssh-keygen -R`。

## [1.4.10] — 2026-10-25

### Changed
- The Pro trial notice under **Preferences → Account** now lists the Pro tools added recently: network storage and iSCSI mounts, Java heap dumps and Arthas, and one-click security fixes.

### Fixed
- **Disk management** no longer reports a read-only bind mount (such as `docker-init` inside a container) as a filesystem the kernel turned read-only.
- In a narrow window (around 1280 px), the "Updated N s ago" note on the Containers, Processes, Disk and Security toolbars no longer overlaps the controls next to it, and the filter box shrinks instead of covering the Stats / Java toggles.

### 中文
- **改进**：「偏好设置 → 账号」里的 Pro 试用提示补上了新增的 Pro 工具：网络存储与 iSCSI 挂载、Java 堆转储与 Arthas、一键安全加固。
- **修复**：磁盘管理不再把只读的 bind 挂载（例如容器里的 `docker-init`）误报为「被内核改为只读」。
- **修复**：窗口较窄（约 1280 像素）时，容器、进程、磁盘和安全检查工具栏上的「N 秒前更新」不再和旁边的控件重叠；筛选框会自动变窄，不再盖住「资源占用」「Java」开关。

## [1.4.9] — 2026-10-25

### Added
- **Toolbox → Security audit** (Linux hosts): a read-only check of the server that ranks what it finds as critical / warning / note / OK, each with the command that fixes it (copy it, or have it typed into the terminal).
  - **Intrusion signs** (shown first): processes named like crypto miners or disguised as kernel threads, programs running from /tmp or /dev/shm, an `/etc/ld.so.preload`, cron jobs and services that download and run scripts, connections to mining-pool ports. These are heuristics: "nothing found" cannot prove a host is clean.
  - **SSH**: root login with a password, password logins, empty passwords, outdated algorithms, weak or recently added keys; failed logins in 24 hours and 7 days with the top IPs, recent successful logins, and whether fail2ban (or similar) is running.
  - **Accounts and sudo**: extra UID 0 accounts, accounts without a password (password hashes never leave the server), passwordless sudo rules, docker group members.
  - **Exposed ports**: Redis, MongoDB, Elasticsearch, the Docker API, MySQL and other services reachable from other hosts, with a table of every exposed port.
  - **Firewall**: whether ufw / firewalld / iptables / nftables is really in effect, and Docker-published ports bypassing ufw.
  - **Updates**: pending security updates (from the local package cache, no download), a newer kernel waiting for a reboot, automatic updates, end-of-life OS versions.
  - **File permissions** (`/etc/shadow`, sudoers, cron, `.ssh`, SUID programs in odd places) and **containers** (privileged, Docker socket mounted, host PID or network).
- **One-click security fixes** (Pro, included in the 14-day trial):
  - **Harden SSH** (password logins off, root keys only, X11 off…) and **Enable ufw** (the SSH port always allowed). Both are checked on the server and roll back on their own after 2 minutes unless you click **Keep**, even if VsTerm lost the connection; open a new connection to check before keeping.
  - Turning password logins off is refused when your current connection logged in with a password, or the user has no SSH key.
  - **Fix** on simple findings (file permissions, locking accounts without a password), after a confirmation.

### 中文
- **新增**：工具箱「安全检查」（Linux 主机），只读检查服务器，把发现的问题分为严重 / 警告 / 提示 / 正常，每项附带修复命令（可复制，或一键填入终端）。
  - **入侵迹象**（排在最前）：疑似挖矿或伪装成内核线程的进程、从 /tmp 或 /dev/shm 运行的程序、`/etc/ld.so.preload`、会下载并执行脚本的定时任务和服务、连接矿池端口的进程。这些是启发式检查，没发现问题也不能保证主机没有被入侵。
  - **SSH**：允许 root 用密码登录、允许密码登录、允许空密码、过时的加密算法、弱密钥或最近新增的公钥；近 24 小时和 7 天的登录失败次数及来源 IP 排行、最近成功的登录，以及是否启用了 fail2ban 等工具。
  - **账户与 sudo**：多余的 UID 0 账户、没有密码的账户（密码哈希不会离开服务器）、免密 sudo 规则、docker 组成员。
  - **对外端口**：Redis、MongoDB、Elasticsearch、Docker API、MySQL 等可被其他主机访问的服务，并列出所有对外端口。
  - **防火墙**：ufw / firewalld / iptables / nftables 是否真的生效，以及 Docker 映射的端口会绕过 ufw。
  - **系统更新**：待装的安全更新（只读本地缓存，不联网下载）、需要重启的新内核、自动更新、已停止维护的系统版本。
  - **文件权限**（`/etc/shadow`、sudoers、定时任务、`.ssh`、异常位置的 SUID 程序）和**容器**（特权容器、挂载 Docker socket、共享宿主机进程或网络）。
- **新增**：一键安全修复（Pro，含在 14 天试用中）：
  - **一键加固 SSH**（关闭密码登录、root 只允许密钥、关闭 X11 转发等）和**启用 ufw**（SSH 端口固定放行）。改完会在服务器上校验，并在 2 分钟后自动恢复原配置，除非你点「保留更改」；即使 VsTerm 断开了也会恢复。保留前请先新开一个连接确认能登录。
  - 如果当前连接是用密码登录的，或该用户没有 SSH 公钥，不允许关闭密码登录。
  - 简单问题（文件权限、锁定没有密码的账户）可以点「修复」，确认后执行。

## [1.4.8] — 2026-10-25

### Added
- **Processes tab** for Linux hosts (over SSH, or the local shell on Linux). It refreshes only while open.
  - List or parent/child tree, sortable columns, search by name, command, PID, user or port, and toggles for "listening only" and kernel threads. CPU %, memory, state (zombie / IO-wait highlighted), uptime and listening ports per process; the processes carrying your own connection are marked. Cut-off text shows in full on hover.
  - Details for the selected process: executable (flagged when the binary was replaced since it started), working directory, full command line, threads, open files against the limit, disk I/O rate and totals, and start time.
  - **Started by**: systemd service (with unit file, ExecStart and state), container, SSH login, tmux / screen, pm2 / supervisord, cron or login session; plus its ancestors and children.
  - Its listening ports, connections and the remote IPs with the most connections.
  - **Environment variables** only on request, with values that look like passwords, tokens or keys masked until revealed.
  - **End process** / **Force kill**, and for systemd services **View logs**, **Restart** and **Stop**, all confirmed first. It warns when a process carries your current connection or a service may restart it.
  - Links: click a process in the side Monitor or a PID in Connections to open it here; open a container process in the Containers tab, or its working directory in the Files panel.
  - Processes running in a container are tagged with the container's name, and search matches it. A new button on each running container in the Containers tab opens its main process here (or the JVM under a shell wrapper).
- **Java diagnostics** for JVMs on the host or in Docker / Podman containers: a Java section in the process details, and **Toolbox → Java diagnostics** to list only JVMs. Every tool runs as the JVM's own user, inside its container when it has one.
  - Version, main class, JVM flags, max heap and collector; heap, old generation, Metaspace and young / full GC refreshed every 5 s, with a 5-minute heap chart and a warning when full GCs keep coming.
  - **Thread analysis**: which threads used the CPU in the last second, matched to their stacks, plus counts by state, deadlocks, every thread with a filter, and the raw dump to copy. Images with only a JRE still show CPU by thread name.
  - **Class histogram** of the classes using the most memory (no full GC; confirmed first, since walking the heap pauses the JVM).
  - **Heap dump** after a free-space check and a confirmation (full GC and a pause): compressed on the server, then downloaded through the Files panel and deleted when done, for analysis in Eclipse MAT or VisualVM.
  - **Arthas** in one click: VsTerm checks the JVM can load it, gets Arthas 4.3.5 (downloaded by the server, or uploaded from this computer when the server has no internet; checksum verified), and starts it in the terminal as the JVM's user. The Java section shows while Arthas is still attached and can stop it.
- **Toolbox → Disk analysis** (Linux hosts): first aid when a disk fills up.
  - Filesystems with usage and inodes; the fullest is selected.
  - **Directories**: drill down by size or file count; a scan covers three levels at once and deeper ones are scanned on click. The largest files of each directory are listed with it.
  - **Large files** (20 MB and up) on a filesystem, with open-in-Files and download.
  - **Deleted but open**: deleted files still holding space because a process keeps them open, with the process (jump to Processes) and a confirmed **Truncate** that frees the space without restarting it.
  - **Cleanup**: the systemd journal (keeps 200 MB), rotated logs under `/var/log`, crash dumps and APT / DNF / YUM / Zypper caches, each showing what it frees and confirmed first; images and build cache link to the Containers tab.
  - Scans run at idle priority with a time limit and can be cancelled. Without root, sizes may be incomplete (it says so).
- **Toolbox → Disk management** (Linux hosts):
  - **Devices**: disks, partitions, LVM and filesystems as a tree, with unallocated space, blank disks and system disks marked. SMART health per drive (temperature, power-on time, bad sectors, SSD wear; sleeping drives are not woken, virtual disks are recognised). Kernel I/O errors of the last day and filesystems the kernel made read-only are flagged.
  - **Speed test**: sequential and 4K random read / write with IOPS and latency (fio; dd when fio is missing). Confirmed first, cancellable, and the test file is always removed.
  - **Swap**: create or remove a swapfile (size suggested from memory; fstab backed up and verified, every step undone on failure) and set swappiness.
  - **fstab check**: entries that would stop a boot, `/dev/sdX` names (with the UUID line to copy), network shares without `nofail`, filesystems mounted but missing from fstab, plus `findmnt --verify`.
  - **Initialise a blank disk**: one GPT partition, ext4 or xfs, mounted and added to fstab by UUID with `nofail`. The device name is typed to confirm, the disk is checked again on the server first, and any failed step is undone.
  - **Grow after a cloud resize**: re-read the disk size, then grow the partition, LVM PV / LV and filesystem (ext2/3/4, xfs, btrfs) online; the partition table is backed up first.
  - **Add a blank disk to an LVM volume group**, optionally growing one LV (and its filesystem) into it; undone on failure.
  - **Network storage**: mount NFS (presets for Alibaba Cloud NAS, Tencent Cloud CFS and AWS EFS), SMB, and S3-compatible object storage through s3fs (Alibaba Cloud OSS, Tencent Cloud COS, Cloudflare R2, AWS S3, MinIO and others). Mounted first, then added to fstab with `_netdev,nofail`; mount or remove them from the list. Passwords and keys go to root-only files and never appear on a command line.
  - **iSCSI**: this host's initiator name, discover the targets on a portal, connect with optional CHAP (and at boot), disconnect (refused while a disk is in use). A connected LUN can then be initialised (with `_netdev`) or added to a volume group.
  - Without smartctl, fio or another tool, it shows the install command to run in the terminal.
- **sudo in VsTerm** for Disk analysis and Disk management: enter the sudo password once per connection and every disk task runs as root. The password stays in memory only and reaches sudo on stdin; the Files panel uses the same password. When you switch to root in the terminal (`sudo -i`), the disk tools say why that does not carry over and offer the button.
- **Pro** (included in the 14-day trial): adding NFS / SMB / object storage mounts, connecting iSCSI targets, adding a disk to an LVM volume group, and the Java class histogram, heap dump and one-click Arthas. Viewing, unmounting, removing, disconnecting and everything else in Processes, Disk analysis and Disk management stay free.

### Changed
- Container log viewer: removed **Follow in terminal**; it added little and could hang VsTerm.
- Disk tools: "Run in terminal" fallbacks are a small terminal icon beside their notice, so they are no longer confused with real actions.

### Fixed
- **Edit in external editor** (Files panel): when saving back fails (for example without write permission), VsTerm now tries 3 times and then stops, instead of retrying forever. Saving again in the editor, or elevating the file panel, tries again.
- Lines pasted in a Windows editor no longer show up as `^M` in `vi`: files that use Unix line endings keep them when saved back.
- A black strip no longer appears between the session list and the main area when the server panel is collapsed.
- **Cancel** on an SFTP download now stops it at once. On slow links it used to wait for every read already requested (about 20 s at 200 KB/s), and it now also cancels the files queued after it. A cancelled transfer says "Download cancelled" instead of "Transfer failed".
- The side Monitor's CPU for SSH hosts now shows current usage, not the average since boot (and since start for each process).

### 中文
- **新增**：「进程」标签页，支持 Linux 主机（SSH 连接，或本机 Linux 的本地 Shell），只在打开时刷新。
  - 列表或父子树两种视图，可按列排序，可按名称、命令、PID、用户、端口搜索，可切换"仅监听端口"和"内核线程"。显示每个进程的 CPU%、内存、状态（僵尸 / IO 等待高亮）、运行时长和监听端口，并标出承载当前连接的进程。显示不全的内容悬停可看全文。
  - 选中进程的详情：程序文件（启动后被替换时会标出）、工作目录、完整命令行、线程数、打开文件数与上限、磁盘读写速率与累计、启动时间。
  - **启动来源**：systemd 服务（含服务文件、ExecStart 和状态）、容器、SSH 登录、tmux / screen、pm2 / supervisord、cron 或登录会话；以及父进程链和子进程。
  - 该进程的监听端口、连接列表，以及连接最多的远端 IP。
  - **环境变量**按需读取，疑似密码、token、密钥的值默认打码，可单独显示。
  - **结束进程** / **强制结束**；systemd 服务可**查看日志**、**重启**、**停止**，操作前都会确认。若进程承载当前连接，或服务可能被自动拉起，会提前提醒。
  - 页面联动：在侧边监控或网络连接页点进程 / PID 可直接跳到这里；容器进程可跳到容器页，工作目录可在文件面板打开。
  - 容器里的进程会标出容器名，搜索也能按容器名查找。容器页每个运行中的容器多了一个按钮，可直接跳到它的主进程（如果是 shell 包装，则直接定位到里面的 JVM）。
- **新增**：Java 诊断，支持宿主机和 Docker / Podman 容器里的 JVM。进程详情里新增「Java」分区，工具箱里新增「Java 诊断」（只列出 Java 进程）。所有操作都以 JVM 自己的用户执行，在容器里的会自动进入容器。
  - 版本、主类、JVM 参数、最大堆和 GC 类型；每 5 秒刷新堆、老年代、Metaspace 和 Young / Full GC，附最近 5 分钟的堆曲线，Full GC 持续增加时会提醒。
  - **线程分析**：找出最近 1 秒占用 CPU 的线程并对应到调用栈，按状态统计，标出死锁，可筛选全部线程、复制原始线程栈。只有 JRE 的镜像也能按线程名查看 CPU。
  - **类直方图**：占用内存最多的类（不触发 Full GC；因为要遍历堆、JVM 会短暂停顿，执行前会确认）。
  - **堆转储**：先检查磁盘空间并确认（会 Full GC 并暂停 JVM），在服务器上压缩后通过文件面板下载，用完可删除，供 Eclipse MAT 或 VisualVM 分析。
  - **Arthas 一键启动**：先检查 JVM 能否加载 Arthas，再准备 Arthas 4.3.5（服务器能上网就直接下载，不能上网则从本机上传，均校验哈希），然后以 JVM 的用户在终端里启动。Arthas 仍挂在 JVM 里时，Java 分区会显示并可一键停止。
- **新增**：工具箱「磁盘分析」（Linux 主机），磁盘满了时的急救工具。
  - 文件系统列表，显示使用率和 inode，默认选中最满的。
  - **目录**：按容量或文件数逐层下钻；一次扫描覆盖三层，更深的目录点开时再扫描；同时列出该目录里最大的文件。
  - **大文件**：列出文件系统上 20 MB 以上的文件，可在文件面板打开或下载。
  - **已删除仍占用**：已删除但仍被进程打开、空间没释放的文件，显示占用进程（可跳到进程页），确认后可**截断**，不用重启进程就能腾出空间。
  - **可清理**：systemd 日志（保留 200 MB）、/var/log 下已轮转的旧日志、崩溃转储、APT / DNF / YUM / Zypper 软件包缓存，每项显示可释放的空间并先确认；容器镜像和构建缓存跳到容器页处理。
  - 扫描以低优先级运行、有时间上限、可取消。没有 root 权限时，结果可能不完整（会提示）。
- **新增**：工具箱「磁盘管理」（Linux 主机）。
  - **设备**：磁盘、分区、LVM、文件系统树形展示，标出未分配空间、未使用的新盘和系统盘。每块盘的 SMART 健康（温度、通电时间、坏扇区、SSD 寿命；不会唤醒休眠的硬盘，能识别虚拟磁盘）。标出近一天的内核 I/O 错误和被内核改为只读的文件系统。
  - **测速**：顺序和 4K 随机读写，显示吞吐、IOPS 和延迟（使用 fio；没有 fio 时用 dd）。先确认、可取消，测试文件一定会删除。
  - **Swap**：创建或删除 swapfile（按内存推荐大小；先备份并校验 fstab，任一步失败都会撤销），设置 swappiness。
  - **fstab 检查**：会导致开机卡住的条目、使用 `/dev/sdX` 设备名（给出可复制的 UUID 写法）、没有 `nofail` 的网络盘、已挂载但未写入 fstab 的文件系统，并附 `findmnt --verify` 结果。
  - **初始化新盘**：建一个 GPT 分区，格式化为 ext4 或 xfs，按 UUID 挂载并写入 fstab（带 `nofail`）。需要输入设备名确认，执行前在服务器上再检查一次，任何一步失败都会撤销。
  - **云盘扩容后扩展**：重新读取磁盘容量，在线扩展分区、LVM 物理卷 / 逻辑卷和文件系统（ext2/3/4、xfs、btrfs），改分区表前先备份。
  - **新盘加入 LVM 卷组**，可选把新空间扩给某个逻辑卷（连同文件系统）；失败时撤销。
  - **网络存储**：挂载 NFS（预设阿里云 NAS、腾讯云 CFS、AWS EFS 的推荐参数）、SMB，以及通过 s3fs 挂载 S3 兼容对象存储（阿里云 OSS、腾讯云 COS、Cloudflare R2、AWS S3、MinIO 等）。先试挂载，成功后才写入 fstab（带 `_netdev,nofail`），列表里可挂载或移除。密码和密钥保存在仅 root 可读的文件里，不会出现在命令行上。
  - **iSCSI**：显示本机发起端名称，发现门户上的目标，连接（可选 CHAP、开机自动连接），断开（磁盘仍在使用时拒绝）。连上的磁盘可以直接初始化（自动带 `_netdev`）或加入卷组。
  - 缺少 smartctl、fio 等工具时，给出安装命令，可填入终端执行。
- **新增**：磁盘分析和磁盘管理支持在 VsTerm 里 sudo 提权：每个连接输入一次 sudo 密码，之后所有磁盘操作都以 root 执行。密码只保存在内存里，通过标准输入交给 sudo；文件面板共用同一个密码。在终端里切到 root（`sudo -i`）时，磁盘工具会说明为什么不能沿用，并提供提权按钮。
- **Pro**（含在 14 天试用中）：添加 NFS / SMB / 对象存储挂载、连接 iSCSI 目标、新盘加入 LVM 卷组，以及 Java 类直方图、堆转储和一键 Arthas。查看、卸载、移除、断开，以及进程、磁盘分析、磁盘管理里的其他功能始终免费。
- **调整**：容器日志窗口去掉「在终端中跟随」，作用不大，还可能卡住 VsTerm。
- **调整**：磁盘工具里的「在终端执行」改为提示旁边的小终端图标，不再和真正的操作按钮混淆。
- **修复**：文件面板「用编辑器打开」保存回写失败（例如没有写权限）时，不再无限重试，最多尝试 3 次后停止；在编辑器里再保存一次，或在文件面板提权后，会重新回写。
- **修复**：在 Windows 编辑器里粘贴的多行文本，保存回 Linux 后在 `vi` 里不再出现 `^M`：原本是 Unix 换行的文件会保持 Unix 换行。
- **修复**：收起服务器面板后，会话列表和主区域之间不再出现黑色竖条。
- **修复**：SFTP 下载点「取消」后立即停止。以前在慢速网络下要等已发出的读请求全部传完（200 KB/s 时约 20 秒）；现在也会一并取消排队中的文件，状态栏显示「已取消下载」而不是「传输失败」。
- **修复**：SSH 主机侧边监控的 CPU 改为显示当前使用率，而不是开机以来（以及进程启动以来）的平均值。

## [1.4.7] — 2026-10-18

### Added
- **Containers tab** for Docker and Podman, on the SSH host (over the session's own connection, falling back to `sudo -n` when the login user cannot reach the daemon) or on this computer. It shows up only when an engine is installed.
  - Containers grouped by compose project, with start / stop / restart / resume / remove, a log viewer, a shell inside the container and follow-logs in the terminal, and optional CPU / memory. Images list which containers use them; unused ones can be removed.
  - **Update check**: compares each image with its registry and flags containers still running an older image after the tag moved.
  - **Safe upgrade** with automatic rollback. Plain containers are recreated from their current settings (environment, ports, volumes, networks, labels, restart policy, limits…), keeping all volume data; compose services go through `compose pull` / `up`. The old container is kept until the new one stays up and healthy, and restored otherwise. The target version can be picked from the newest 10 tags of the same variant, or typed in. Containers whose options cannot be reproduced safely (`--rm`, `--volumes-from`, links…) are refused rather than guessed.
  - **Ports**: forward a container port to this computer through the SSH connection, with nothing opened on the server — published ports or, for unpublished ones, the container's own address. Forwards also show in the SSH tunnels panel and close with the connection. Local containers open in the browser.
  - **Disk**: usage by images, containers, volumes and build cache, with confirmed cleanups (stopped containers, dangling or unused images, build cache, anonymous volumes, or a safe all-in-one cleanup).
  - Local engines: Docker Desktop, OrbStack, Rancher Desktop, Podman and Homebrew installs are found on macOS; on Windows the engine is called directly (update check and upgrades are not available there yet). A stopped engine is reported as such instead of as a permission problem.

### 中文
- **新增**：「容器」标签页，管理 Docker / Podman。支持 SSH 主机（复用当前会话连接，登录用户无权限时自动尝试 `sudo -n`）和本机，只有装了容器引擎时才显示。
  - 容器按 compose 项目分组，可启动 / 停止 / 重启 / 恢复 / 删除，查看日志，一键进入容器终端或在终端里跟踪日志，可选显示 CPU / 内存；镜像列表显示被哪些容器使用，未使用的可删除。
  - **检查更新**：对比镜像仓库，标出可更新的镜像，以及标签已更新但容器还在用旧镜像的情况。
  - **安全升级**，失败自动回滚：普通容器按原有配置（环境变量、端口、卷、网络、标签、重启策略、资源限制等）重建，卷数据全部保留；compose 服务走 `compose pull` / `up`。新容器稳定运行且健康后才删除旧容器，否则自动恢复。可以从同类标签中最新的 10 个版本里选择目标版本，或手动输入。无法安全复刻的容器（`--rm`、`--volumes-from`、links 等）会直接说明原因，不做猜测。
  - **端口**：通过 SSH 连接把容器端口转发到本机，服务器上不用开放任何端口；已发布端口和未发布端口（经容器地址）都支持。转发同时显示在 SSH 隧道面板里，断开连接自动关闭。本机容器可直接在浏览器打开。
  - **磁盘**：镜像、容器、数据卷、构建缓存的占用和可回收空间，并提供需确认的清理操作（已停止容器、悬空 / 未使用镜像、构建缓存、匿名卷，或一键安全清理）。
  - 本机引擎：macOS 会自动找到 Docker Desktop、OrbStack、Rancher Desktop、Podman 和 Homebrew 安装的命令；Windows 直接调用引擎（暂不支持检查更新和升级）。引擎没启动时会明确提示，而不是显示成没有权限。

## [1.4.6] — 2026-10-16

### Changed
- **Refreshed app icon.** Same V and three colours, tidied up: a subtle gradient plate with a faint rim so the icon keeps its outline on dark taskbars and docks, a heavier V, and the three blocks turned into one tri-colour cursor bar. 16–32 px sizes (taskbar, tabs, favicons) now use their own simplified artwork — a thicker V and a single cursor bar — instead of a scaled-down copy, and the macOS icon sits on the standard Apple icon grid.

### 中文
- **调整**：更新应用图标。保留 V 字和三种配色，细节重新打磨：底板改为轻微渐变并加一圈淡色描边，在深色任务栏 / 程序坞上也能看清轮廓；V 字加粗，三个色块合成一条三色光标条。16–32 px（任务栏、标签页、网站图标）改用专门的简化稿（更粗的 V + 单条光标），不再由大图直接缩小；macOS 图标按 Apple 标准图标网格放置。

## [1.4.5] — 2026-10-14

### Changed
- **Route Info topology reads left to right**, the same way as the proxy / VPN lanes: LAN segments → this host → WAN / VPN / proxy TUN uplinks → Internet. The legend follows the same order.
- **Redesigned SSH tunnels panel** (plug button, top right) to match System Info and Route Info: each tunnel is a card with its type, listen → destination address, status chip and start / stop / copy-SOCKS buttons; errors show under the row, the footer shows how many tunnels are up, and rows keep a stable order.

### Removed
- **Policy graph** view in Route Info. Its content is covered by the Overview topology and the policy-rules card, so Route Info now has Overview and Route tables only.

### 中文
- **调整**：路由信息的网络拓扑改为从左到右，与代理 / VPN 链路方向一致：局域网 → 本机 → WAN / VPN / 代理 TUN → 互联网，图例顺序同步调整。
- **调整**：重新设计右上角「SSH 隧道」面板，与系统信息、路由信息风格一致：每条隧道一张卡片，显示类型、监听 → 目标地址、状态标签，以及启动 / 停止 / 复制 SOCKS 地址按钮；出错信息显示在卡片下方，底部显示运行数量，列表顺序固定。
- **移除**：路由信息中的「策略图」视图。其内容已由「概览」拓扑和路由规则卡片覆盖，路由信息现在只保留「概览」和「路由表」。

## [1.4.4] — 2026-10-14

### Added
- **Proxy and VPN links in the Route Info topology.** Under the network diagram, each proxy (sing-box, Clash / mihomo, Xray, V2Ray, Hysteria, TUIC, Shadowsocks, Trojan, NaiveProxy) and VPN (WireGuard, OpenVPN) now gets its own lane: LAN devices or local apps → how traffic is captured → the proxy → the remote servers it connects to → WAN → Internet.
  - **Capture**: TUN device, TPROXY (iptables mangle or nftables), REDIRECT (nat) and DNS hijack to the proxy's DNS port, matched against the ports the proxy listens on.
  - **Remote servers**: worked out from the proxy's own public connections, sorted by connection count; the busiest one is marked as the node and drawn as a dashed tunnel, the rest fold into "other direct targets". LAN, fake-IP and CGNAT addresses are ignored. WireGuard peers show as online when the last handshake is under 3 minutes old.
  - Costs one more capped `ss` (or `netstat`) call inside the existing single probe.

### Changed
- Refreshed README screenshots.

### 中文
- **新增**：路由信息网络拓扑中的代理 / VPN 链路。拓扑图下方为每个代理程序（sing-box、Clash / mihomo、Xray、V2Ray、Hysteria、TUIC、Shadowsocks、Trojan、NaiveProxy）和 VPN（WireGuard、OpenVPN）单独画一条链路：局域网设备或本机程序 → 流量接管方式 → 代理程序 → 远程服务器 → WAN → 互联网。
  - **接管方式**：识别 TUN 网卡、TPROXY（iptables mangle 或 nftables）、REDIRECT（nat）以及劫持到代理 DNS 端口的 DNS，并与代理监听的端口核对。
  - **远程服务器**：根据代理自身连向公网的连接统计，按连接数排序；连接最多的标为「节点」，用虚线画出隧道，其余合并为「其他直连目标」。忽略内网、fake-IP 和 CGNAT 地址。WireGuard 对端 3 分钟内有握手即显示在线。
  - 只在原有的单次探测里多一条限制了行数的 `ss`（或 `netstat`）查询。
- **调整**：更新 README 截图。

## [1.4.3] — 2026-10-14

### Added
- **Route Info overview for routers and servers.** Route Info now opens on an Overview with three views (Overview, Route tables, Policy graph). It recognises OpenWrt (including ImmortalWrt / iStoreOS), Asuswrt-Merlin / stock Asuswrt and plain Linux, and draws a topology diagram: Internet → WAN / VPN / proxy TUN uplinks → this host → LAN segments with how many clients are online.
  - **Proxies & VPN**: sing-box, Clash / mihomo / OpenClash, Xray, V2Ray, Hysteria, TUIC, Shadowsocks, Trojan, NaiveProxy, OpenVPN, Tailscale, ZeroTier, frp and AdGuard Home / SmartDNS / MosDNS, with their listening ports and TUN device. WireGuard peers show endpoint, allowed IPs, last handshake and traffic.
  - **DHCP clients** (dnsmasq leases on OpenWrt, Merlin and most Linux) with online state and lease time left, **interfaces** with their role and bridge ports, and custom **policy rules**.
  - **Route tables** are grouped by table (main, custom tables, local) and fold; a filter box searches destination, gateway, interface and table.

### Changed
- **Redesigned System Info, Connections, Route Trace, IP Quality and Software Sources** with the same cards, tables, inputs and buttons as the rest of the app, in light and dark themes. System Info has summary tiles and disk usage bars; Connections' charts are clickable filter cards over a smooth table that handles thousands of rows; Route Trace shows the path with latency bars and loss; IP Quality has a risk gauge and side-by-side cards; Software Sources uses switches and setting rows.
- **Lighter route probe**: still one SSH command per refresh, with every section capped so a busy router stays idle, and auto-refresh every 30 s instead of 8 s. WireGuard is read without ever fetching private keys.

### 中文
- **新增**：路由信息「概览」。路由信息分为「概览」「路由表」「策略图」三个视图。自动识别 OpenWrt（含 ImmortalWrt / iStoreOS）、华硕梅林 / 原厂固件和普通 Linux，并画出网络拓扑：互联网 → WAN / VPN / 代理 TUN → 本机 → 各局域网，局域网显示在线客户端数。
  - **代理与 VPN**：识别 sing-box、Clash / mihomo / OpenClash、Xray、V2Ray、Hysteria、TUIC、Shadowsocks、Trojan、NaiveProxy、OpenVPN、Tailscale、ZeroTier、frp，以及 AdGuard Home / SmartDNS / MosDNS，显示监听端口和 TUN 网卡；WireGuard 显示对端、允许的 IP、上次握手和流量。
  - **DHCP 客户端**（OpenWrt、梅林和多数 Linux 的 dnsmasq 租约）显示是否在线和租约剩余时间；**网络接口**显示角色和桥接成员；以及自定义**路由规则**。
  - **路由表**按 main、自定义表、local 分组，可折叠，并可按目标、网关、接口或表名筛选。
- **调整**：重新设计「系统信息」「网络连接」「路由追踪」「IP 质量检测」「软件源」，与软件其他部分使用同样的卡片、表格、输入框和按钮，浅色深色主题都适配。系统信息加了概要卡片和磁盘使用率条；网络连接的统计图可点击筛选，连接表几千条也流畅；路由追踪以路径形式显示延迟条和丢包；IP 质量有风险仪表和并排卡片；软件源改用开关和设置行。
- **调整**：路由探测更轻量：每次刷新仍只执行一条 SSH 命令，每段输出都限制了行数，繁忙的路由器也不受影响；自动刷新从每 8 秒改为每 30 秒。WireGuard 信息不会读取私钥。

## [1.4.2] — 2026-10-14

### Changed
- **Redesigned Add / Edit Server dialog.** Fields are grouped into Connection, Authentication, Group & appearance and Advanced cards. Host and port share a row, the proxy is picked from a field that shows "Direct" when none is set, and password vs. private key is a two-way switch. A "From vault" button fills the login from saved credentials (double-clicking the username still works). Saving the secret is a switch, with a notice when no master password is set. Tag colours are round swatches, OS icons a tile grid, and Save / Cancel stay pinned at the bottom. The leftover proxy hint line under the form is gone.
- **Server tree**: the Servers / Monitor switch, the Server / Folder / Local shell buttons and the search and collapse buttons use the new style. Rows have a rounded hover and selection highlight. Folders show how many servers they hold; server rows show the host and the colour tag on the right.
- **Monitor panel**: CPU, memory and swap use rounded bars that turn amber above 75 % and red above 90 %. Processes, network and storage each sit in a titled card. Numbers are right-aligned, busy processes are highlighted, the traffic chart is filled, and every disk gets a small usage bar.
- **Main window**: Terminal / System Info / Routes / Connections and Files / Commands are underlined tabs with icons. Toolbar and menu-bar buttons are flat with a soft hover, and the remote path field matches the other inputs.
- **Dark themes**: primary buttons on light accent colours (e.g. Tokyo Night) use dark text so the label stays readable.

### 中文
- **调整**：重新设计「添加 / 编辑服务器」对话框。内容分成「连接」「认证」「分组与外观」「高级」四张卡片；主机和端口放在同一行；代理改为选择框，未设置时显示「直连」；密码 / 私钥改为分段切换。新增「从凭据库选择」按钮（双击用户名仍然可用）。「保存到凭据库」改为开关，没有设置主密码时会给出提示。标签颜色改为圆形色块，图标改为方块网格，「保存 / 取消」固定在底部。表单下方残留的代理提示语已删除。
- **调整**：服务器列表。「服务器 / 系统监控」切换、「服务器 / 文件夹 / 本地 Shell」按钮以及搜索、收起按钮换成新样式；行悬停和选中有圆角高亮；文件夹显示服务器数量，服务器行右侧显示主机地址和标签颜色。
- **调整**：系统监控。CPU、内存、交换区改为圆角进度条，超过 75% 变黄、超过 90% 变红；进程、网络、存储分别放进带标题的卡片；数字右对齐，占用高的进程会标色；流量图带填充，每块磁盘有使用率小条。
- **调整**：主窗口。「终端 / 系统信息 / 路由信息 / 网络连接」和「文件 / 命令」改为带图标的下划线标签；工具栏和菜单栏按钮改为扁平样式，悬停有柔和底色；远端路径输入框与其他输入框风格一致。
- **调整**：深色主题下，浅色强调色（如 Tokyo Night）上的主按钮改用深色文字，保证可读。

## [1.4.1] — 2026-10-14

### Changed
- **Redesigned Preferences pages** (General, Master Password, Account, Cloud Sync). Every page now uses the same cards with an icon header, the same button sizes, and setting rows with the text on the left and the control on the right. On/off options are switches. Status shows as small coloured labels. Light and dark themes both follow the active theme colours.
  - **Cloud Sync**: Sync and Compare sit side by side. The two overwrite actions moved into a Replace menu, each with a line explaining what it does. Per-kind status and local backups show as tidy lists, and only kinds that differ show when the cloud copy changed.
  - **Account**: the plan, email and cloud storage use are on one card, and the expiry date no longer appears twice. Devices are a list that shows when each was last active.
  - **Master Password**: the new password and its confirmation sit side by side.

### 中文
- **调整**：重新设计偏好设置的「常规」「主密码」「账号」「云端同步」四个页面。所有页面统一使用带图标标题的卡片和同样尺寸的按钮；设置项左边是说明、右边是控件；开关类选项改成开关按钮；状态用小的彩色标签显示。浅色和深色主题都跟随当前主题配色。
  - **云端同步**：「同步」「与云端比较」并排放在一起；两个覆盖操作收进「覆盖」菜单，每项附一句说明。各类型的状态和本机备份改为整齐的列表，只有有差异的类型才显示云端更新时间。
  - **账号**：套餐、邮箱和云端存储用量放在同一张卡片里，到期时间不再重复显示。设备改为列表，显示每台设备最近活跃的时间。
  - **主密码**：新密码和确认密码并排显示。

## [1.4.0] — 2026-10-13

### Added
- **Two-way cloud sync** (Options → Preferences → Cloud Sync → Sync). A change made on one device is copied to the other. When both devices changed the same kind, they are merged instead of one overwriting the other: sessions, the server tree, quick commands and credential entries are merged item by item, and deletions carry over. If the same value was changed differently on both, this device's value is kept and the cloud version is saved in a local backup. The first sync on a device that has never synced only adds, it never deletes. Upload and Download still replace one side with the other.
- **Compare with cloud**: shows, per kind, whether this device, the cloud, or both changed since the last sync, without transferring anything.
- **Local backups and restore**: before a sync, upload, download or restore replaces local files, they are saved under `sync-backups/` in the data directory (last 30 kept). An upload over cloud data this device had not seen also saves that cloud version first. Cloud Sync lists the backups, and each can be restored, including the cloud version that was replaced. A restore backs up what it replaces, so it can be undone. Backups work offline and while signed out.

### Changed
- **Faster sync**: requests reuse one connection instead of a new TLS handshake each time (a sync made over a dozen). A kind that changed on neither side is skipped, so a sync with nothing new is a single request. The account and device list are no longer re-fetched on every sync.
- **Saved logins and proxies now sync**: the credentials kind also carries `credentials/catalog.yaml` and `proxies/catalog.yaml`. They hold `vault://` references only, never secrets. Older versions ignore these files.
- **Network timeouts**: cloud requests give up after 15 s to connect and 60 s without data, instead of waiting forever.

### 中文
- **新增**：云端双向同步（选项 → 偏好设置 → 云端同步 →「同步」）。只在一台设备上改过的内容会同步到另一台；两台都改过同一类内容时会合并，而不是互相覆盖。会话、服务器列表、快捷命令和凭据条目逐项合并，删除也会同步过去。同一个值两边改得不一样时保留本机的值，云端那份存进本机备份。从没同步过的设备第一次同步只会补充内容，不会删除任何东西。「上传到云端」「从云端下载」仍然是用一边整体覆盖另一边。
- **新增**：「与云端比较」：不传输任何数据，按类型显示自上次同步以来是本机改了、云端改了，还是两边都改了。
- **新增**：本机备份与恢复。同步、上传、下载或恢复替换本机文件之前，会把这些文件存到数据目录下的 `sync-backups/`，保留最近 30 次。上传覆盖了本机没见过的云端数据时，也会先把那份云端数据存下来。云端同步页列出所有备份，每个都可以恢复，包括当时被替换掉的云端版本。恢复前会先备份当前文件，所以恢复也能撤销。备份在离线或未登录时也能用。
- **调整**：同步更快。所有请求复用同一个连接，不再每个请求都重新握手（一次同步原来要握手十几次）。两边都没变的类型直接跳过，没有新内容时一次同步只需要一个请求。也不再每次同步都重新拉取账号和设备列表。
- **调整**：已保存的登录凭据和代理现在也会同步：凭据类型同时带上 `credentials/catalog.yaml` 和 `proxies/catalog.yaml`。这两个文件只有 `vault://` 引用，不含密码。旧版本会忽略它们。
- **调整**：云端请求加了超时：连接超过 15 秒、或 60 秒没有数据就放弃，不再无限等待。

## [1.3.9] — 2026-10-13

### Added
- **Cloud Bun desk pet** (Pro): a chibi girl with cloud-bun hair, 1.5× the loong (Options → Effects → Desk Pet). Idle, every 6–8 seconds, she tosses a small cloud from her hand and catches it. Typing holds a shy pose. Enter and connecting send five clouds once around her while she sways by at most 10° and keeps facing the camera. Between those beats the window sleeps. Software rendering stays on the existing ~15 FPS cap; hardware uses ~30 FPS only during the toss and the orbit, and ~5 FPS while the window is in the background. Position saved as `desk_pet_cloud_x` / `desk_pet_cloud_y`.

### Fixed
- **Cursor lag when holding arrow keys**: after a fast run of Left / Right, the cursor could keep moving after the key was released, or stop one cell behind the shell until the next output. Two causes. SSH connections left Nagle's algorithm on, so each keystroke packet waited for the previous one's ACK and keys piled up on slower links. The socket now sets `TCP_NODELAY`, like OpenSSH. The terminal view's frame cache could also pair an old screen with output that arrived mid-frame, so it kept drawing the old cursor position.

### Changed
- **Less CPU per frame**: the terminal reuses each character's laid-out glyph across frames instead of laying out every visible cell again. A full 200×50 screen now takes about 0.8 ms per frame instead of 2 ms. The command-suggestion popup ranks history once per typed query instead of every frame (up to 0.6 ms per frame with 10,000 commands). Each frame now copies only new command blocks into history, and the History button copies them only while its list is open. With the idle screen lock on, the vault file is re-read every 2 seconds instead of every frame. A session file that cannot be read or written is no longer retried every frame while its OS icon is saved.

### 中文
- **新增**：桌宠「云团」（Pro）——云团双丸子头的 Q 版女孩，大小是青金龙的 1.5 倍，不是金佛、太上老君那种两倍（选项 → 特效 → 桌面宠物）。闲置时每隔 6 到 8 秒，从手里抛起一朵小云再接住。打字换成害羞的捂脸，按住约半秒，连续按键不会把重绘钉在高帧率。回车和连上主机时，五朵云绕她转一整圈，身体最多歪 10 度，脸始终朝向屏幕。两段动作之间窗口睡觉。软件渲染沿用现有的大约 15 帧上限；硬件渲染只在抛云和绕圈时大约 30 帧，窗口在后台时大约 5 帧。位置记在 `desk_pet_cloud_x` / `desk_pet_cloud_y`。
- **修复**：连续快速按左右方向键时，松开按键后光标还会继续移动，或者停在比实际位置落后一格的地方，直到下一次有输出才跟上。原因有两个：SSH 连接没有关闭 Nagle 算法，每个按键包都要等上一个包的确认才发出，网络稍慢时按键会堆积，现在像 OpenSSH 一样开启了 `TCP_NODELAY`；终端画面的缓存可能把旧画面和这一帧中途到达的输出记在一起，于是一直画着旧的光标位置。
- **调整**：降低每帧的 CPU 占用。终端每个字符的字形排版结果会跨帧复用，不再每帧把屏幕上所有字符重新排版一遍，整屏 200×50 从每帧约 2 毫秒降到约 0.8 毫秒。命令提示弹窗只在输入内容变化时重新排序历史，不再每帧都排（1 万条历史时每帧最多约 0.6 毫秒）。每帧只把新增的命令块记入历史，工具栏的「历史」按钮也只在列表打开时才读取。开启空闲锁屏后，保险库文件改为每 2 秒读一次，不再每帧读取。保存主机系统图标时，会话文件读写失败也不会再每帧重试。

## [1.3.8] — 2026-10-07

### Changed
- **Command suggestions**: opening the popup no longer selects a row, so Enter submits the command you typed. Up and Down still choose a suggestion, and clicking a row still fills it. Hovering a history row shows a trash icon at its end; clicking it removes that command from the local history.
- **Command history**: short commands that are not worth recalling are no longer stored. That includes `ls`, `cd`, `mkdir`, `rm`, `pwd`, `clear`, and the same commands with arguments or a `sudo` prefix, plus a bare word such as `git` or `top`. A pipeline, `&&` / `;` chain, redirect, or `$()` is still saved, for example `cd /srv && make`. Commands already stored that match this rule are dropped the next time history loads.

### 中文
- **调整**：命令提示弹出时不再默认选中一行，直接按回车会提交你正在输入的命令。上下键仍然用来选择建议，点击某一行仍然填入。鼠标移到历史命令行尾会出现删除图标，点击后从本机历史里删掉这条命令。
- **调整**：太短、不值得再提示的命令不再记录，包括 `ls`、`cd`、`mkdir`、`rm`、`pwd`、`clear`，以及它们带参数或前面加 `sudo` 的写法，还有单独一个词的 `git`、`top` 等。管道、`&&`、`;`、重定向和 `$()` 仍会保存，例如 `cd /srv && make`。已经存过、又符合这条规则的命令，会在下次加载历史时清掉。

## [1.3.7] — 2026-10-07

### Fixed
- **macOS Keychain prompts after an update**: the first launch of a new version asked for the Keychain password several times, once per stored item, even after "Always Allow". The app is now signed with a fixed certificate, so "Always Allow" carries over to later updates. Launch also no longer reads three of those items: the command-history key comes from its local key file, the Pro trial uses a new item, and the cloud device key and sync key load only when needed. Updating to this version can still ask once if "Remember unlock" is on, plus twice more if you are signed in to a cloud account. Later updates should not ask again.

### 中文
- **修复**：macOS 升级后第一次打开时会连续弹出好几次钥匙串密码框（每个存储的条目弹一次），点了「始终允许」下次升级还会再弹。现在程序改用固定证书签名，「始终允许」在以后的升级中会一直有效。启动时也不再读取其中三个条目：命令历史的密钥改从本地密钥文件读取，Pro 试用改用新条目，云设备密钥和同步密钥改为用到时才读取。升级到本版本时，如果开启了「记住解锁」仍可能弹 1 次，已登录云账号的再多 2 次；之后的升级不会再弹。

## [1.3.6] — 2026-10-06

### Added
- **Golden Buddha desk pet** (Pro): a seated golden statue, twice the size of the loong (Options → Effects → Desk Pet). It stays still. Idle and typing keep a soft light around the head; Enter (and connecting) widens that light over the whole figure, then lets it settle. The light is transparent and fades out at the edge, so the window shows through. No projectile. Position saved as `desk_pet_buddha_x` / `desk_pet_buddha_y`.
- **Taishang Laojun desk pet** (Pro): a seated sage (Options → Effects → Desk Pet). Idle and typing light the eight trigrams one by one on the wheel behind him. Enter and connecting light all eight. Purple qi drifts around the figure; the yin-yang turns slowly. The figure itself stays still. Position saved as `desk_pet_laojun_x` / `desk_pet_laojun_y`.
- **14-day Pro trial**: every install can use the local Pro features — the larger scrollback cap and the Monkey, Golden Buddha, and Laojun pets — for 14 days from first launch. Afterwards they need VsTerm Pro; a Pro pet that is in use switches back to the dog. The desk pet menu and Preferences → Account show the days left. Cloud sync still requires a Pro entitlement.

### Fixed
- **Laojun CPU use**: the pet kept the whole window repainting at 30 FPS. It now repaints at 10 FPS, and 5 FPS while the window is in the background. With software rendering, idle CPU drops from about 58% of a core to 37% (18% in the background).
- **Locked Pro pets**: hovering a locked pet in the menu now shows why it is locked. The hint never appeared before.

### 中文
- **新增**：桌宠「金佛」（Pro）——金色坐像，大小是青金龙的两倍（选项 → 特效 → 桌面宠物）。人像始终不动。闲置和打字只有头部一层渐渐淡出的金光；回车和连上主机时金光扩大到全身，边缘淡出，再慢慢收回。金光是透明的，后面的窗口能透出来。不发射光球。位置记在 `desk_pet_buddha_x` / `desk_pet_buddha_y`。
- **新增**：桌宠「太上老君」（Pro，选项 → 特效 → 桌面宠物）。闲置和打字时，背后八卦逐个点亮；回车和连上主机时八个全亮。周身紫气缓慢飘动，太极缓慢旋转，人物保持静坐。位置记在 `desk_pet_laojun_x` / `desk_pet_laojun_y`。
- **新增**：14 天 Pro 免费试用。从首次启动起 14 天内，可以免费使用本地 Pro 功能：更大的回滚行数上限，以及猴子、金佛、太上老君桌宠。到期后需要开通 VsTerm Pro；正在使用的 Pro 桌宠会切回小狗。桌宠菜单和「偏好设置 → 账号」会显示剩余天数。云同步仍需要 Pro 权益。
- **修复**：太上老君会让整个窗口按 30 FPS 持续重绘，现在改为 10 FPS，窗口在后台时 5 FPS。软件渲染下空闲 CPU 从约 58% 单核降到 37%（后台 18%）。
- **修复**：鼠标悬停在锁定的 Pro 桌宠上时会显示原因，以前这个提示从不出现。

## [1.3.5] — 2026-09-29

### Added
- **Drag to reorder the server list**: drag a server or a folder. The top or bottom edge of a row inserts before or after that item; the middle of a folder moves the item into it. While dragging something out of a folder, a bar at the top of the list drops it at the end of the root. The order is saved in `tree.yaml`. Folders still nest at most two levels, and a folder that already has a subfolder cannot be nested further. Dragging is off while the list is filtered, because hidden rows would make the drop index wrong.

### Fixed
- **Command suggestions while recalling history**: pressing Up or Down to recall a previous command no longer opens the suggestion popup, so the next arrows stay with the shell. The popup still opens when you type, paste, or delete on the prompt. Once it is open, Up and Down still move the selection.

### 中文
- **新增**：服务器列表可以拖动排序。拖到某一行的上沿或下沿，插到该项前面或后面；拖到文件夹行中间，移进这个文件夹。从文件夹里往外拖时，列表上方会出现「放到根目录」，松手后放到根目录末尾。顺序写入 `tree.yaml`。文件夹仍然最多嵌套两层，已经有子文件夹的不能再套进别的文件夹。搜索过滤开着的时候不能拖，因为这时看到的不是完整列表。
- **修复**：按上下键翻历史命令时不再弹出命令提示，后续的上下键继续发给终端。打字、粘贴、退格或删除时仍会出现提示；提示已经打开时，上下键仍用来选择建议。

## [1.3.4] — 2026-09-22

### Fixed
- **Chinese paste and input**: a single-line paste is sent as the characters themselves, so the shell does not paint the line in standout. On Windows the local PowerShell keeps its colors when the console is switched to UTF-8. A remote shell that has no UTF-8 locale (including BusyBox on Merlin / Dropbear) is started in `C.UTF-8`, and bash is told to echo multibyte input, so pasted and typed Chinese show up on the command line. `echo` of the same text already worked.
- **Copying Chinese out of the terminal**: a CJK glyph occupies two cells, and the trailing cell was copied as a space. Pasting that back produced `你 好`. On Windows PowerShell, `echo` splits on those spaces, so each character printed on its own line. The spacer cell is skipped; a space you actually typed is kept.
- **SFTP drag onto the terminal**: a path is quoted only when it is empty or contains whitespace or shell metacharacters. `/etc/hosts` and `/home/文档` are inserted as-is. A name with spaces or quotes is still one single-quoted argument.

### 中文
- **修复**：粘贴和输入中文。单行粘贴直接送字符，不再触发整行反白。本机 PowerShell 切到 UTF-8 时保留原来的颜色。远端如果没有 UTF-8 locale（包括梅林上的 BusyBox / Dropbear），启动时补上 `C.UTF-8`，bash 也会回显多字节输入，所以粘贴和输入的中文会出现在命令行上。`echo` 打出的中文本来就是对的。
- **修复**：从终端复制中文。汉字占两格，后半格以前被当成空格复制出去，贴回去就变成「你 好」。Windows 上 PowerShell 的 `echo` 会按这些空格拆参数，于是一个字一行。现在跳过占位格；你自己输入的空格仍会保留。
- **修复**：拖到终端的路径只在为空、含空格或含 shell 元字符时加引号。`/etc/hosts`、`/home/文档` 原样插入；带空格或引号的名字仍然是一个单引号参数。

## [1.3.3] — 2026-09-19

### Added
- **SFTP Copy Path**: right-click a remote file or folder (list or directory tree) to copy its full path to the clipboard. Multi-select copies every selected path, one per line, so they can be pasted into a terminal or another editor.
- **SFTP drag onto the terminal**: drag a remote row onto the terminal pane to insert the quoted path at the shell's current input. Several selected items become space-separated quoted paths; names with spaces or quotes stay one argument.
- **Scrollback memory budget**: terminal history is now bounded by memory rather than by line count alone. A history row costs roughly 24 bytes per column, so the old 100 000-line ceiling meant ~205 MB at 80 columns but ~490 MB at 200 — per tab, with nothing watching. Preferences → General sets a per-terminal budget (32 / 64 / 128 / 256 / 512 MiB, default 64, or unlimited for the previous behaviour) and shows the live total; the terminal's Clear scrollback submenu shows the active tab's figure. The budget is re-applied when a window is widened, because the same line count costs more once rows get wider.

### Changed
- **Startup memory**: ~63 MiB less resident at idle on a machine with a system CJK UI face (282 MB → 218 MB measured on Linux; Windows and macOS always take that path). Three causes: every owned font face was resident twice because `FontData::from_owned` makes egui clone the bytes again for its rasterizer, the app loaded three CJK faces regardless of UI language, and `Vault::try_open` re-ran a 19 MiB Argon2id pass on startup and two or three more times per connect just to test whether a master password is set. Faces are now handed over as `'static`, only the primary CJK face is loaded, and the password probe is cached against the current DEK wrap.
- **Connect latency**: opening a session no longer stalls on two or three Argon2id derivations (tens of milliseconds each).
- **Monitor sampling**: the process table no longer resolves every process's executable path and disk usage once a second — it only shows pid, name, CPU and memory.

### Fixed
- **Imported Windows key paths** are shown by file name again on macOS and Linux. The credential picker asked `Path::file_name` for the last component, which only knows the host's separator, so a path like `C:\Users\vesaa\.ssh\id_ed25519.pem` was treated as one long name and truncated from the drive letter (`C:\Users\vesaa\.s….pem`). Xshell, FinalShell and SecureCRT imports keep `UserKey` paths verbatim, so this showed up on any cross-platform import.

### Removed
- **Korean UI language**. Existing `locale: ko` configs fall back to English. Japanese is unaffected: YaHei / PingFang SC / Noto Sans SC all cover kana, so the separate Japanese face only changed kanji from SC to JP shapes and was not worth 10–20 MB.

### 中文
- **新增**：SFTP 右键「复制路径」——文件列表或左侧目录树右键即可把完整远端路径写入剪贴板；多选时一行一条，方便贴到终端或其它编辑器。
- **新增**：把 SFTP 文件/文件夹拖到终端区域，会在当前输入位置插入带引号的路径；多选则空格分隔，含空格或引号的名字仍是一个参数。
- **新增**：终端回滚缓冲区改为按内存限制，而不再只看行数。一行历史约占「24 字节 × 列数」，所以原来的 10 万行上限在 80 列下是约 205 MB，200 列下就是约 490 MB —— 而且是每个标签页各算一份，此前没有任何约束。「选项 → 偏好设置 → 常规」可以设置每个终端的上限（32 / 64 / 128 / 256 / 512 MiB，默认 64，也可选不限制以保持旧行为），旁边显示所有标签页的实时合计；终端右键的「清理滚动历史」子菜单里显示当前标签页的占用。窗口拉宽后会重新收敛，因为行变宽之后同样的行数会更贵。
- **变更**：启动常驻内存下降约 63 MiB（Linux 实测 282 MB → 218 MB；Windows 和 macOS 必然走同一条路径）。三个原因：每个系统字体面在内存里存了两份（`FontData::from_owned` 会让 egui 为光栅化再克隆一份）；不论界面语言都加载三个 CJK 字体面；`Vault::try_open` 仅仅为了判断有没有主密码，就在启动时跑一次 19 MiB 的 Argon2id，每次连接再跑两三次。现在字体面按 `'static` 交给 egui、只加载主 CJK 面、密码探测按当前密钥封装缓存。
- **变更**：连接会话不再卡在两三次 Argon2id 推导上。
- **变更**：监视器进程表不再每秒为每个进程解析可执行文件路径和磁盘用量。
- **修复**：macOS / Linux 上导入的 Windows 私钥路径重新按文件名显示。凭证选择器用 `Path::file_name` 取末段，而它只认宿主平台的分隔符，于是 `C:\Users\vesaa\.ssh\id_ed25519.pem` 被当成一个超长文件名，从盘符开始截断成 `C:\Users\vesaa\.s….pem`。Xshell / FinalShell / SecureCRT 导入会原样保留 `UserKey` 路径，所以任何跨平台导入都会碰到。
- **移除**：韩语界面。已有配置里的 `locale: ko` 会回落到英文。日语不受影响：雅黑 / 苹方 / Noto Sans SC 本身都含假名，单独的日文面只改变汉字字形（SC 形 → JP 形），不值 10–20 MB。

## [1.3.2] — 2026-09-17

### Added
- **Inline command suggestions**: after a couple of characters, matching local history (and saved quick commands) appears in a popup above the caret. Arrow keys / click / Enter fill the remainder without running it. History is encrypted on disk (`cmdhist.enc`), never cloud-synced, and commands that look like they contain secrets are dropped.

### Fixed
- **Command history truncation**: wrapped long commands are joined when recording (toolbar History and the new suggestions), so the stored text is the full line rather than the first row. Overlong rows in both panels ellipsize instead of overflowing.
- **CJK vs ASCII cell alignment**: each terminal glyph is optically centered in the cell using its ink (`Galley::mesh_bounds`). `LEFT_CENTER` only centers the shared layout box, so Chinese still hugged the top while digits/Latin sat mid-cell ([#22](https://github.com/vesaaa/vsterm/issues/22)). CJK is the OS UI face (PingFang SC on macOS, YaHei on Windows, Noto on Linux) — macOS does not use YaHei.
- **Suggestion “quick” tag**: the CJK/Latin label is laid out as its own centered galley instead of sharing a LayoutJob with the monospace command, so it stays on the same baseline (Windows 125% DPI).

### 中文
- **新增**：终端输入联想。打几个字符后，本地历史（以及快捷命令）在光标上方弹出；方向键 / 点击 / Enter 填充剩余字符，不执行。历史加密保存在本机，不同步云端；像密码、令牌的命令不会入库。
- **修复**：折行的长命令现在会拼成完整一条再记入右上角历史和新的联想；超长文本在面板里省略，不再撑破宽度。
- **修复**：同一行里中文靠顶、英文/数字居中（[#22](https://github.com/vesaaa/vsterm/issues/22)）。按每个字形的墨水盒垂直居中，而不是居中整行 layout 盒。macOS 中文是苹方，不是雅黑。
- **修复**：联想列表里「快捷」不再和命令错行（Win10 125% DPI 下中英混排基线不一致）。

## [1.3.1] — 2026-09-16

### Added
- **SSH client proxy**: reusable SOCKS5 / HTTP CONNECT catalog (same pattern as credentials). Add/Edit Server has one proxy row (default None); picking a proxy opens the list. Manage multiple named proxies via Options → Proxy Library. Direct TCP is unchanged; VsTerm does not use the OS / `HTTP_PROXY` system proxy for SSH. SOCKS5 sends the hostname to the proxy (remote DNS). OpenSSH `ProxyCommand` (`nc`/`ncat`/`connect`/`corkscrew`), Xshell, and FinalShell proxy fields are imported into the catalog.

### Changed
- **Proxy Library**: each saved proxy can be edited and tested (SOCKS5 method negotiation, or HTTP CONNECT). Leave the password blank when editing to keep the saved secret. List chips sit next to row-height icon actions (test / edit / delete); Credential Vault delete uses the same icon button, plus an eye to peek at a password. **Show passwords** next to Add Credential is remembered in `config.yaml` until unchecked, and reveals every secret in the vault list and in the Add Server credential picker.

### 中文
- **变更**：添加/编辑服务器只保留一行选择代理（默认「无」）；点击打开代理列表，双击选用。多条代理在「选项 → 代理库」里配置，交互与凭证库一致。代理库支持编辑与测试（SOCKS5 握手 / HTTP CONNECT）；编辑时密码留空则保留已保存密码。代理库列表加宽贴齐操作按钮，测试/编辑/删除改为与行等高的图标；凭证库列表铺满宽度，删除为图标，左侧增加查看密码（眼睛）；「添加凭证」旁可勾选「不隐藏密码」（写入配置，取消勾选前一直生效），凭证库与添加主机的凭证列表都会明文显示。
- **新增**：会话级 SOCKS5 / HTTP CONNECT 代理（与 Termius 的 Proxy 同类）。直连行为不变；SSH **不走** 系统代理或 `HTTP_PROXY`。SOCKS5 把主机名交给代理做远程 DNS。可从 OpenSSH `ProxyCommand`、Xshell、FinalShell 导入到代理库。

## [1.3.0] — 2026-09-15

### Changed
- **Release CI**: compile cache is sccache on the dedicated `R2_SCCACHE_*` bucket (`ci/sccache/`), not the CDN download bucket. v1.2.14 did not run sccache; this release is the first tag that should write cache objects. The first build is still a cold compile (`cache writes`); later tags with the same rustc/deps should hit.

### 中文
- **变更**：发版编译缓存改走独立的 `R2_SCCACHE_*` 桶（前缀 `ci/sccache/`）。v1.2.14 当时还没合进这段 workflow；这一发会第一次写入缓存（冷编译），下一发才可能命中。

## [1.2.14] — 2026-09-15

### Added
- **Software sources (toolbox)**: switch Debian/Ubuntu apt and RHEL-family yum/dnf mirrors (official, Aliyun, Tsinghua/USTC EDU, 163, Tencent, Huawei), plus optional Docker CE and Nginx *package* repos. Each apply renames originals on the host to `<name>.bak.<UTC>` (existing backups are kept) and can restore that file. OpenWrt/Merlin/Alpine/Windows are detected and left untouched.

### Changed
- **Software sources**: hide the empty VsTerm-backup hint; preview and current source files use a smaller muted monospaced panel; privilege follows a terminal `sudo -i` / `su` the same way the file panel does.
- **About**: lists the official site [www.vsterm.com](https://www.vsterm.com) alongside GitHub.
- **Account entitlements**: last successful `GET /v1/promo/channels` is cached on disk so the list paints immediately; a skeleton placeholder shows while the first fetch is in flight, then the live list replaces it.
- **Buy Pro**: Preferences → Account purchase action (and the unsigned fallback row) opens [https://www.vsterm.com/vsbuy](https://www.vsterm.com/vsbuy).

### Fixed
- **SFTP vs `sudo -i`**: typing `sudo -i` no longer pops the file-panel sudo password dialog (that stole the password from the PTY). The password goes to the terminal; SFTP stays the login user until you click Elevate (or when `sudo -n` is already enough).
- **Software sources after `sudo -i`**: Apply no longer wraps a new exec with `sudo -n` (which fails with `sudo: a password is required` because tty_tickets do not share the PTY timestamp). The script runs in the already-elevated terminal instead.

### 中文
- **变更**：软件源无备份时不再显示空说明；预览/当前源文件改为浅色小号等宽文本框；终端里 `sudo -i` 后与文件面板一样按提升后的身份探测与写入。
- **新增**：工具箱「软件源」——切换 Debian/Ubuntu apt 与 CentOS 7 / Rocky / Alma / Stream 的 yum/dnf 镜像（官方、阿里云、清华/中科大 EDU、163、腾讯、华为），可选 Docker CE / Nginx **软件包**仓库。每次应用在服务器把原文件改名为 `原名.bak.UTC时间戳`（不覆盖已有备份），可按备份恢复。OpenWrt / 梅林 / Alpine / Windows 只提示不支持，不改写。
- **关于**：补充官网地址 [www.vsterm.com](https://www.vsterm.com)。
- **开通权益**：上次成功拉取的渠道列表会缓存在本地，打开账号页先渲染缓存；没有缓存时用加载占位，服务端返回后再替换。
- **购买 Pro**：偏好配置 → 账号 可直接打开购买页，也可访问 [https://www.vsterm.com/vsbuy](https://www.vsterm.com/vsbuy)。
- **修复**：普通用户在终端输入 `sudo -i` 时，SFTP 提权窗口不再抢走密码；密码先给终端。SFTP 保持登录用户，需要时再点文件面板的提权（免密 sudo 仍会自动跟随）。
- **修复**：终端 `sudo -i` 成 root 后，软件源界面虽显示 root，点保存不再报 `sudo: a password is required`。写入改在已经提权的终端里执行（新的 SSH exec 仍是登录用户，且 tty_tickets 不能把 PTY 的 sudo 票据带到 exec）。

## [1.2.13] — 2026-09-09

### Fixed
- **SSH to Dropbear / older Linux**: enabling Shell integration no longer fails the whole login. The OSC bootstrap is ~10 KB `exec`; Dropbear’s command cap is ~9000 bytes and a longer string often **disconnects the session** (so retrying on another channel cannot recover). VsTerm now skips that exec on Dropbear / unknown banners and opens a normal login shell. If integration still fails after a fallback, the error is **Shell integration failed** (uncheck OSC path sync) — not a generic host/port/network failure. Password auth asks the server which methods it offers. Handshake also offers NIST ECDH, `diffie-hellman-group14-sha1`, `hmac-sha1`, and AES-CBC after the modern algorithms.
- **Account sign-in**: the verification-code field and Sign in button no longer sit on top of each other (the preferences shell zeros item spacing). They share one row with an 8px gap.

### Changed
- **Preferences window**: the dialog stays a fixed size but can be dragged by the title bar (it is no longer re-anchored to the center every frame).
- **Preferences action spacing**: Browse/Clear, Refresh/Sign out, Upload/Download, and the three device cards now use a 2px gap (same as the session-tree search/collapse icons).
- **Device cards**: left OS badge (Windows / macOS / Linux), right column with hostname on top and Revoke / “This device” aligned to the bottom-right.
- **Cloud upload guard**: uploading with fewer than 5 local sessions asks for confirmation (and to verify the local config is current) so a small local tree cannot silently overwrite cloud config.

### 中文
- **修复**：部分 Linux（尤其是梅林 / OpenWrt 等 Dropbear）勾选 OSC 路径同步后会提示「连接失败」，其它 SSH 软件却能连。集成脚本约 10KB，超过 Dropbear 命令上限时会直接掐掉整条会话。现在对 Dropbear 会改走普通登录 shell，勾选 OSC 也能进终端。若集成仍然失败，会单独提示「Shell 集成失败」（取消勾选 OSC），不再误导成主机/端口/网络不通。密码登录会先询问服务器支持的认证方式；算法协商补上 NIST ECDH、`diffie-hellman-group14-sha1`、`hmac-sha1`、AES-CBC。
- **修复**：账号页发送验证码后，验证码输入框与登录按钮不再重叠（偏好设置外壳把控件间距设为 0）；两者同一行并留 8px。
- **变更**：偏好设置窗口尺寸仍固定，可用标题栏拖动（不再每帧钉在屏幕正中）。
- **变更**：偏好设置里成对按钮（浏览/清除、刷新/退出登录、上传/下载）以及「我的设备」三张卡片间距统一为 2px。
- **变更**：设备卡片改为左 OS 图标、右主机名；「撤销」与「当前设备」都放在右下角。
- **变更**：本地服务器少于 5 台时，上传到云端会先确认（并提醒确认本地配置是最新的），避免误覆盖云端配置。

## [1.2.12] — 2026-09-08

### Changed
- **Native preferences window**: Options → Preferences is now a settings center with a square icon sidebar (General / Master password / Account / Cloud sync), vertical cards, and a fixed dialog size. Title bar and shadow match the main window.
- **External editor command line**: Preferences → General accepts a typed or pasted editor path, including extra flags (Notepad++ `-multiInst`, VS Code `--wait`, `%f` / `{file}` placeholders). A missing or crashing editor only shows an error and does not block the rest of VsTerm. Browse/Clear on that row use a 10px gap.

### 中文
- **变更**：偏好设置改为原生设置中心：左侧图标侧栏（常规 / 主密码 / 账号 / 云端同步）、纵向卡片、固定窗口尺寸；标题栏和阴影与主窗口一致。
- **变更**：外部编辑器支持手动输入/粘贴，并可带启动参数（如 Notepad++ `-multiInst`、VS Code `--wait`，以及 `%f` / `{file}`）。程序不存在或启动失败只提示错误，不影响其它功能。浏览/清除间距为 10px。

## [1.2.11] — 2026-09-07

### Added
- **Jade loong desk pet**: original teal-gold coiled Chinese loong (Options → Effects → Desk Pet). Idle pose matches the concept (tail coiled on the left, one paw on the ground pearl); typing uses a tiny keyboard; Enter opens the mouth and the pearl flies from the snout (same projectile as the qi fighter). Free tier; position saved as `desk_pet_dragon_x` / `desk_pet_dragon_y`.
- **SFTP external editor**: “Open with editor” downloads the remote file as raw bytes, opens a user-chosen editor, and uploads the saved bytes on disk change. No encoding or newline conversion. The editor path lives in machine-local `vsterm.conf` (next to the binary when writable, otherwise the platform config dir) and is **not** cloud-synced. First use prompts for an editor; **Options → Preferences → General** can set or clear it.

### Fixed
- **Host import encoding**: MobaXterm (and other ANSI session files) from Chinese Windows no longer show mojibake such as `Éú²ú` for `生产`. Files without a BOM are decoded as UTF-8 when valid, otherwise GB18030 / Big5 when the text looks like CJK, and Windows-1252 as a last resort. WindTerm, Xshell, FinalShell, Tabby, OpenSSH, and SecureCRT use the same decoder.

### 中文
- **新增**：桌宠「青金龙」——原创青绿金角螭龙（选项 → 特效 → 桌面宠物）。闲置盘尾在左、一爪踩地珠；打字用小键盘；回车张嘴，珠从嘴前飞出（与气功武者同一弹道）。免费；位置记在 `desk_pet_dragon_x` / `desk_pet_dragon_y`。
- **新增**：SFTP「使用编辑器打开」按字节下载到临时文件，用用户指定的外部编辑器打开，保存后按字节回写远端（不做编码/换行转换）。编辑器路径写在本机 `vsterm.conf`（能写则放程序目录，否则平台配置目录），**不同步云端**。首次使用会选择编辑器；也可在 **选项 → 偏好设置 → 常规** 里设置或清除。
- **修复**：从中文 Windows 的 MobaXterm（以及其他 ANSI 会话文件）导入时，`生产` 不再显示成 `Éú²ú`。无 BOM 文件优先 UTF-8，否则按 GB18030 / Big5 识别中文，最后才回退 Windows-1252。WindTerm / Xshell / FinalShell / Tabby / OpenSSH / SecureCRT 使用同一解码。

## [1.2.10] — 2026-09-06

### Fixed
- **Remote macOS shell integration**: connecting with Shell integration enabled no longer fails with `/bin/sh: syntax error near unexpected token ;;`. macOS `/bin/sh` is bash 3.2, which treats `)` in `case pat)` inside `$(...)` as the end of the substitution. Last-login scanning now keeps `case` in a helper function, and uses BSD `last -5` (with GNU `last -n 5` as fallback).
- **CJK font RSS**: system `.ttc` collections load a single extracted face instead of the whole file (macOS PingFang / Apple SD Gothic / Hiragino, Windows YaHei, Linux Noto CJK). On this machine PingFang dropped from ~74 MB to ~13 MB in-process.

### 中文
- **修复**：开启 Shell 集成连接远端 macOS 时，`/bin/sh`（bash 3.2）报 `syntax error near unexpected token ;;`。Last login 探测把 `case` 挪出 `$(...)`，并兼容 BSD `last -5`。
- **修复**：系统 TTC 字体只抽取当前字重进内存，不再整包加载 PingFang / 雅黑 / Noto CJK 合集。

## [1.2.9] — 2026-09-05

### Changed
- **Account sign-in**: the unsigned-in Account tab now explains that Pro subscription, cloud sync, and device management require sign-in, and that the email only receives a login verification code (no password). SSH connections do not need a cloud account.

### Fixed
- **macOS language menu**: Korean (`한국어`) no longer renders as tofu. PingFang SC has no Hangul coverage; the UI font stack now falls back to Apple SD Gothic Neo (and Hiragino for Japanese), matching the existing Windows Malgun / Yu Gothic fallbacks. Linux loads Noto Sans CJK KR when present.
- **OS keyring prompts**: cloud session fields (token, expiry, account id/email, device id, fingerprint, auth-block) are stored in a single `cloud-session` item instead of ~7 separate entries. Existing split items are migrated on first read. Vault “remember unlock” and the device Ed25519 key stay separate; in-process caches skip repeat `get_password` calls (macOS / Windows / Linux). Signing in writes the session blob once.
- **macOS Network Connections**: GUI-launched apps often have no `/usr/sbin` on `PATH`, so `netstat` was never found. BSD `netstat -an` also uses `tcp4`/`udp6` and `host.port` instead of Linux `tcp` / `host:port`, so every chart stayed at 0. The collector now locates `netstat` the same way path-trace finds `traceroute`, and the parser accepts the BSD rows.
- **macOS Path Trace**: Auto/IPv4 against a literal such as the default `8.8.4.4` passed GNU-only `-4` to Apple traceroute (`invalid option -- 4`), so the panel showed “未能解析到路径跳点”. The probe now drops address-family flags the local tool rejects, merges traceroute stderr, and still parses classic BSD `host (ip)` / `host [ip]` hop lines.
- **Path Trace mtr**: report mode no longer passes `-z`. That ASN column replaces the `|---` host marker, so every hop rendered as `*` and extra IPv4 continuation lines became fake hop numbers. The parser also accepts `AS???` / `ASnnnn` rows if `-z` is present.
- **Private-key picker** no longer filters to `.key` / `.pem` / `.ppk`. OpenSSH keys without a suffix (`id_rsa`, `id_ed25519`, self-built names) are selectable. A **Paste** button next to Browse fills a file path from the clipboard.

### 中文
- **改进**：未登录账号页补充说明：登录后才可订阅 Pro、使用云端同步并管理设备；邮箱仅用于接收登录验证码，无需设置密码。SSH 连主机不依赖云账号。
- **修复**：macOS 多语言菜单中韩文不再乱码；在 PingFang SC 后追加 Apple SD Gothic Neo（日文追加 Hiragino）。
- **修复**：云会话相关钥匙串合并为一条 `cloud-session`，启动时不再连续弹出约 7 次授权。Vault 记住解锁与设备密钥仍分条保存；进程内缓存避免重复读取。登录只写一次会话条目。Windows 凭据管理器 / Linux Secret Service 同样减少读写。
- **修复**：macOS「网络连接」TCP/UDP 图表一直为 0。从 Finder 启动时 `PATH` 常不含 `/usr/sbin`（`netstat` 在那里）；即便采到输出，解析器也不认 BSD 的 `tcp4` / `192.168.1.1.443`。现已按绝对路径找 `netstat` 并解析该格式。
- **修复**：macOS「路由追踪」对 IPv4 字面量（默认 `8.8.4.4`）会把 GNU 的 `-4` 传给 BSD `traceroute`，报 `invalid option -- 4`，界面只显示「未能解析到路径跳点」。现会探测工具是否接受该参数，并合并 stderr、解析 BSD 跳点行。
- **修复**：路径追踪的 mtr 报告不再带 `-z`（ASN 列会吃掉 `|---`，跳点地址全变成 `*`）。解析器同时兼容 `AS???` 行。
- **修复**：私钥「浏览」不再只显示 `.key` / `.pem` / `.ppk`；无后缀密钥（`id_rsa`、自建文件名）可选。路径旁新增「粘贴」，用于粘贴文件路径。

## [1.2.8] — 2026-09-03

### Fixed
- **macOS release packaging** now ad-hoc signs the assembled `.app`, verifies the signature in CI, and stages the bundle with `ditto` so Apple Silicon no longer hits the fatal “damaged” path for unsigned builds.
- **Remote Bash shell integration** now trims stray leading/trailing semicolons and whitespace before installing the `PROMPT_COMMAND` hook, avoiding the repeated `syntax error near unexpected token ';'` prompt spam seen on hosts with profiles like `history -a;`.
- **Shell hook installation hardening**: Bash array `PROMPT_COMMAND` is deduped element-wise and prepended so `$?` stays accurate; Zsh fallback hook arrays now dedupe too.

### 中文
- **修复**：macOS 发版打包现在会对组装后的 `.app` 做 ad-hoc 签名，在 CI 中校验签名，并用 `ditto` 进入 DMG 暂存目录，避免 Apple Silicon 把未公证构建直接判成“已损坏”。
- **修复**：远端 Bash 的 shell 集成在安装 `PROMPT_COMMAND` hook 前会清理首尾多余分号和空白，避免 `history -a;` 这类配置导致每次提示符前重复报 `syntax error near unexpected token ';'`。
- **修复**：Shell hook 安装加固：Bash 数组形式的 `PROMPT_COMMAND` 按元素去重并前置，确保 `$?` 保持正确；Zsh fallback 的 hook 数组也会去重。

## [1.2.7] — 2026-09-02

### Added
- **Terminal config** on the host toolbar: toggle gutter **time stamps** and **line numbers** (default on); choices persist in `config.yaml` and cloud preferences sync.
- **Server-tree search** via a filter icon on the left rail.
- **Status bar** shows measured renderer FPS (GPU and software rendering paths).

### Changed
- **SSH connect**: tighter timeouts and clearer failure feedback in the auth dialog.
- **Host import** (WindTerm, Xshell, etc.) runs on a background thread so the UI stays responsive.

### Fixed
- Left sidebar layout: black bar bleed and related tree UX polish.

### 中文
- **新增**：主机工具栏 **终端配置**，可开关 gutter **时间**与**行号**（默认开启），写入 `config.yaml` 并参与云端偏好同步。
- **新增**：左侧会话树 **搜索**（工具栏图标）。
- **新增**：状态栏显示实测渲染 **FPS**（GPU / 软件渲染）。
- **改进**：SSH 连接超时与认证失败提示；主机列表导入改为后台线程。
- **修复**：左侧栏黑边与树交互细节。

## [1.2.6] — 2026-09-01

### Added
- **In-app update checks** now prefer the official CDN manifest at `download.vsterm.com` (Cloudflare R2), with GitHub Releases as fallback; **Download update** opens a platform-matched direct link (zip / dmg / tar.gz).
- **Release CI** syncs stable builds to R2 (`vsterm/latest.json` + per-version binaries); pre-release tags (`v*-beta*`) publish GitHub pre-releases only — no CDN or public `main` sync.

### Changed
- Update-check dialog: **Download update** replaces the old “open download page” primary action; release page remains as a secondary link.

### 中文
- **新增**：检查更新优先走官方 CDN（`download.vsterm.com`），失败回退 GitHub；**下载更新**按本机平台打开直链。
- **新增**：正式版发版 CI 同步产物到 R2；`-beta` 标签仅发 GitHub 预发布，不更新 CDN。
- **改进**：更新对话框主按钮改为「下载更新」。

## [1.2.5] — 2026-09-01

### Added
- **Seven built-in UI themes** under **Options → Theme**: Light, Light-Black, Tokyo Night, Catppuccin Mocha, Nord, Kanagawa Wave, and Everforest Dark. Monitor process/storage rows and gauge labels follow the active theme.
- **Collapsible left rail**: collapse the servers / system-monitor column to a narrow strip via an outline chevron; state persists in `config.yaml` (`left_rail_collapsed`).
- **Credential vault** under **Options → Credential Vault…**: manage reusable SSH passwords and private keys; secrets stay encrypted in `vault.enc`, metadata in `credentials/catalog.yaml`.
- **Session editor**: double-click **Username** to open a credential picker panel beside the dialog; double-click a credential to fill auth fields. Saving a server profile upserts matching credentials when vault save is enabled.

### Changed
- Add/edit server auth form: password and private-key fields align on one row with labels; dialog size stays fixed when switching auth mode.
- Main window opens centered on the work area (accounts for title-bar chrome).

### Fixed
- **Windows**: SSH password login no longer freezes the whole UI after submit.
- **Windows**: local-shell ConPTY sizing — avoid the 2×2 column default and post-output shrink that caused wrap/flash (including 4K high-DPI cell sizing).

### 中文
- **新增**：**选项 → 主题** 内置 7 套主题（Light、Light-Black、Tokyo Night、Catppuccin Mocha、Nord、Kanagawa Wave、Everforest Dark）；监控页进程/磁盘行与仪表标签随主题配色。
- **新增**：左侧**服务器/系统监控列可折叠**（线型三角按钮），状态写入 `config.yaml` 的 `left_rail_collapsed`。
- **新增**：**选项 → 凭证库…**，管理可复用 SSH 密码/私钥；密文存 `vault.enc`，元数据在 `credentials/catalog.yaml`。
- **新增**：添加/编辑服务器时双击**用户名**打开右侧凭证选择面板，双击凭证自动填充；勾选保存时同步写入凭证库。
- **改进**：认证表单标签与输入框同行对齐，切换密码/私钥时对话框尺寸不变；主窗口在工作区内居中启动。
- **修复**：Windows 下 SSH 密码登录提交后不再卡死整个界面；本地 Shell ConPTY 尺寸/高 DPI 下避免异常换行与闪烁。

## [1.2.4] — 2026-08-27

### Added
- Import host lists under **File → Import from Others**: **WindTerm** (`.wind` / `user.sessions`), **Xshell** (`Sessions` / `.xsh`), **FinalShell** (data dir or `conn/`), **MobaXterm** (`MobaXterm.ini` / `.mxtsessions`), **Tabby** (`config.yaml`), **OpenSSH** (`~/.ssh/config`), and **SecureCRT** (`Sessions` / `.ini`). Folder names become groups (two levels). SSH/SFTP only; encrypted passwords are not imported. A private-key *file path* keeps public-key auth.

### 中文
- **新增**：菜单「文件 → 从其他导入」，子项 WindTerm / Xshell / FinalShell / MobaXterm / Tabby / OpenSSH / SecureCRT。只导入 SSH；分组最多两层；密码因对方加密无法导入。私钥文件路径会保留公钥登录。

## [1.2.3] — 2026-08-20

### Changed
- Commands tab menus, new-folder / add-command dialogs, and command-body editing stay responsive: stable chip hit targets, no per-frame command-body clones, and fixed-size editors that do not re-center while typing.

### Added
- Right-click empty space in the command list (below folder chips) to add a command to the current folder.

### 中文
- **改进**：命令页右键、新建文件夹、添加/编辑命令更跟手（固定对话框、避免每帧克隆命令正文）。
- **新增**：命令区空白处右键可添加命令。

## [1.2.2] — 2026-08-12

### Added
- Menu **Check for Updates…** under About: queries public GitHub Releases and reports up-to-date / newer version / failure in a fixed-size dialog.

### Fixed
- Pin production promo signing key `k1` so verified `/v1/promo/channels` drives entitlement list copy (no longer stuck on local legacy fallback without a pin).
- CTA hover text comes from channel `hint` / details / description; omit tooltip when the server sends none.
- Legacy entitlement fallback (unsigned channels) shows only purchase + redeem placeholders — GitHub Star is server-channel only.

### 中文
- **新增**：菜单「检查更新…」，对照公开 GitHub Releases；结果对话框固定尺寸。
- **修复**：钉入生产验签公钥 `k1`，动态权益列表可用；按钮悬停文案跟接口；legacy 兜底仅保留购买/兑换码。

## [1.2.1] — 2026-08-12

### Fixed
- macOS release builds: pin `ip2region` to a resolvable git revision (upstream force-pushed `master` and dropped the old lock SHA) and fetch git deps via CLI to avoid libgit2 SSL failures on runners.

### Docs
- README Standard vs Pro feature table, including Personal Cloud sync crypto notes (Ed25519 device key pair; AES-256-GCM + master password for sync blobs).

### 中文
- **修复**：macOS 发版构建钉死可用的 `ip2region` git 提交，并用 CLI 拉取依赖，避免上游 force-push / SSL 导致 Apple 构建失败。
- **文档**：README 补充标准版 / Pro 对比与云同步加密说明。

## [1.2.0] — 2026-08-12

### Added
- Personal Cloud: signed account session, entitlement channels, redeem, device list, and encrypted vault sync (push/pull with live per-kind progress).
- Local Pro gates: scrollback free 100k / Pro 500k; monkey desk pet requires Pro (dog remains free).
- Desk pets float freely over the whole window (`desk_pet_x`/`desk_pet_y`, `desk_pet_dog_x`/`desk_pet_dog_y`); edge-era configs migrate to near-edge defaults.

### Changed
- GitHub Star OAuth prefers an account picker (`prompt=select_account`) and clears sticky `login=` hints.
- Account prefs: Pro identity strip, dynamic promo actions, email-code rate-limit cooldown with countdown.

### 中文
- **新增**：Personal Cloud（签名会话、权益渠道、兑换、设备列表、加密 vault 同步与分项进度）；本机 Pro 门控（滚动缓冲 / 猴子宠物）；桌宠整窗自由拖放。
- **改进**：GitHub Star OAuth 账号选择；账号页 Pro 身份条与验证码冷却倒计时。

## [1.1.17] — 2026-08-06

### Fixed
- Command History continues after interactive elevation (`sudo -i`, `su`, …). Parent-shell OSC 133 had latched off client Enter marks, while the nested login shell usually has no integration — history stopped. Elevation now re-enables client Enter, and OSC C coalesces with a just-stamped client block to avoid duplicates.

### Docs
- Cloud API contract base URL and response shapes aligned with the public `/sync` gateway (`me.limits`, presigned `required_headers`, download revision metadata).

### 中文
- **修复**：`sudo -i` / `su` 等交互提权后继续记录命令历史（嵌套 shell 常无 OSC 集成，提权后重新启用本机 Enter 打点）。
- **文档**：同步 Cloud API 合同（`/sync` base、`limits`、预签名 `required_headers`、download meta）。

## [1.1.16] — 2026-08-05

### Fixed
- Terminal focus no longer steals arrow / Tab / Escape keys for egui widget navigation after the first press, so shell history recall, Tab completion, and repeated cursor movement stay on the PTY instead of jumping to the History toolbar button.
- Application Cursor Keys (DECCKM) are now encoded from the live emulator mode: bare arrows / Home / End send SS3 (`ESC O A` …) when vi and other fullscreen apps enable `CSI ?1h`, matching xterm terminfo (`kcuu1=\EOA`).

### 中文
- **修复**：终端持焦时锁定方向键 / Tab / Escape，不再被 egui 焦点导航抢走（避免 ↑ 打开「历史记录」）；按模拟器 DECCKM 状态编码应用光标键，vi 等全屏程序中方向键 / Home / End 可用。

## [1.1.15] — 2026-08-04

### Fixed
- Public-key connect dialog no longer starts verifying the moment the first character of an empty username is typed. Auto-verify now requires the username **and** key path to have been prefilled from the saved session; anything typed by hand waits for Verify & Connect. The "Verifying public key…" line only appears when auto-verify will actually run.
- Private key paths are cleaned before use: surrounding quotes (Explorer "Copy as path") and stray whitespace are stripped, and `~\` expands like `~/` on Windows. Existence is checked with `is_file`, so a directory no longer passes as a key.
- "Private key file not found" now names the path that was actually checked, instead of leaving the resolved value invisible.

### Added
- **Browse…** button beside the private key field in both the connect dialog and the session editor, opening a native file picker in the directory already entered.
- Near-miss key suggestions: when a path does not resolve but the same location holds that name with `.key` / `.pem` / `.ppk` / `.openssh`, or a `.pub` was given instead of the private half, the real file is offered as a one-click button. Nothing is substituted silently, and the scan only runs on edit or failed verification.

### Changed
- Saved sessions keep `~` unexpanded, so a session file stays portable when the data directory lives on OneDrive / iCloud / SMB.

### 中文
- **修复**：公钥登录对话框不再在用户名输入第一个字符时就自动验证——仅当用户名与私钥路径均来自已保存会话时才自动发起；私钥路径去除首尾引号与空白，支持 `~\`，存在性检查改用 `is_file`，错误提示带上实际检查的路径。
- **新增**：私钥路径旁「浏览…」按钮（连接对话框与会话编辑器）；路径解析不到时，提示同位置补全 `.key`/`.pem`/`.ppk`/`.openssh` 后的真实文件（或 `.pub` 对应的私钥），点击填入，不做静默替换。
- **改进**：会话保存时保留 `~` 不展开，数据目录放在 OneDrive / iCloud / SMB 时仍可跨机器复用。

## [1.1.14] — 2026-08-04

### Fixed
- Path Trace: IPv6 targets now use family-correct tooling (`-4`/`-6`, `traceroute6` / `tracert -6`, Auto / IPv4 / IPv6 mode); dual-stack hosts no longer silently produce empty hop lists for IPv6.
- IP Quality: public IPv6 probe pins address family (`curl -6` / remote script equivalents) with fallbacks so dual-stack endpoints that advertise A records are not misread as IPv4-only.

### Changed
- Idle password auth dialog no longer forces a continuous redraw loop; software renderers keep a solid caret to avoid unnecessary whole-window paints.
- Terminal PTY output and Monitor / System Info snapshot updates repaint at the renderer cadence (or when data lands) instead of uncapped or fixed polling, cutting high CPU on software adapters under load or idle dialogs.

### 中文
- **修复**：路径追踪 IPv6 目标使用对应族探测与 Auto/IPv4/IPv6 模式；IP 质量本机/远端公网 IPv6 探测正确绑定 IPv6，避免被误判为无 IPv6。
- **改进**：空闲认证弹窗与终端输出重绘按渲染节奏合并，显著降低软件渲染下的 CPU 占用。

## [1.1.13] — 2026-07-28

### Changed
- Net connections: remote location uses the same cascade as path trace (ipinfo → api.vsterm.com → ip2region), filled asynchronously so the table stays responsive; auto-refresh every 5s.
- Net connections charts: hover shows a point and Y value (no vertical crosshair); X-axis shows up to three `mm:ss` sample times.
- SSH: enable TCP keepalive (idle ~60s, probe every 10s) alongside the existing 30s application-layer keepalive.
- Geo/location lookups skip CGNAT `100.64.0.0/10` (e.g. Tailscale) in addition to RFC1918 private ranges.

### 中文
- **改进**：网络连接归属地与路径追踪同级联查询，后台补全；自动刷新改为 5 秒；图表悬停显示折线点与 Y 值，X 轴最多三个分:秒时间点。
- **改进**：SSH 增加 TCP keepalive；归属地查询跳过 CGNAT `100.64/10`。

## [1.1.12] — 2026-07-27

### Changed
- Routes diagram: one chain per policy table (prefer that table’s default), show `ip rule` source (e.g. `from …`) and metrics on exit nodes; remove the unused click-detail text under a selected spoke.

### 中文
- **改进**：图形路由按策略表合并链路、展示规则来源与 metric；去掉点击链路后下方无用的详情文本。

## [1.1.11] — 2026-07-27

### Added
- Routes panel **diagram** view: hub-and-spoke graph for default/policy routes, Linux TPROXY / mark chains (e.g. fwmark → table → lo → TPROXY port / listener), and LAN gateway + NAT detection from `ip rule` / `iptables` / `ss`/`netstat`/`/proc`.
- Terminal selection persists across scroll; **Shift+click** extends the selection across scrollback.

### Fixed
- Terminal: Ctrl+C copy no longer freezes the UI (clipboard write was re-entering the egui input lock).

### 中文
- **新增**：路由面板「图形」视图（默认/策略路由、TPROXY/mark 链路、局域网网关与 NAT）；终端选区滚动后保留，可用 Shift+点击扩展。
- **修复**：终端 Ctrl+C 复制不再卡死。

## [1.1.10] — 2026-07-27

### Fixed
- Windows: opening IP Quality no longer flashes two console windows when probing local public IPv4/IPv6 via `curl` (`CREATE_NO_WINDOW`).

### 中文
- **修复**：Windows 上打开 IP 质量检测时，本地探测公网 IPv4/IPv6 不再弹出两个控制台黑框。

## [1.1.9] — 2026-07-27

### Changed
- IP Quality: public IPv4/IPv6 chips reflect the **active host** — remote SSH runs `curl`/`wget` on the server; Local Shell probes this machine; disconnected hosts show an unavailable state.
- Fraud score bar and risk badges follow Scamalytics `scamalytics_risk` (e.g. score 25 → medium/yellow), not equal 0–100 thirds.

### 中文
- **改进**：IP 质量检测的公网 IPv4/IPv6 标签随当前连接变化（SSH 远端在服务器上探测、本地 Shell 探测本机、断开时不可用）。
- **改进**：欺诈分进度条与风险色块对齐 Scamalytics 风险等级。

## [1.1.8] — 2026-07-27

### Added
- Toolbox **IP Quality Check**: queries `api.vsterm.com` fraud data (risk score bar, operator/line, location, IP type, cloud vendor identity, blacklists, bots, proxies).
- Local public IPv4/IPv6 chips next to Check (via `curl` / `ipv4.ip.sb` · `ipv6.ip.sb`); click to fill the target field.
- Path Trace is toolbox-only (no longer a top tab); Toolbox also keeps Traceroute.

### Changed
- IP Quality layout polish: paired ISP/Org and postcode/timezone rows; IP-type chips (yes-only, centered); vendor / blacklist chip grids.

### 中文
- **新增**：工具箱「IP质量检测」（欺诈分、运营商、地理位置、IP 类型、大厂身份、外部黑名单、Bot、代理）；本机公网 IPv4/IPv6 标签可点击填入；路径追踪仅保留工具箱入口。
- **改进**：IP 质量面板排版（双字段对齐、类型色块居中、黑名单方框+尾部色块等）。

## [1.1.7] — 2026-07-27

### Changed
- Path Trace default target is `8.8.4.4` (was `www.baidu.com`).
- CI: public `vesaaa/vsterm` Release and docs sync only after **all** platform builds succeed (draft staging; incomplete builds are aborted).

### 中文
- **改进**：路径追踪默认目标改为 `8.8.4.4`；Release CI 等全部平台构建成功后再公开发布与同步文档，避免缺包。

## [1.1.6] — 2026-07-26

### Added
- Preferences: when setting a master password for the first time, **Screen lock** is checked by default (and persisted after a successful set).
- Preferences → Data directory: help text notes that the default folder is `.vsterm`; the path field supports typing/paste and a right-click Copy/Paste menu.
- Fresh installs default the desk pet to the **bottom-edge dog**.

### Changed
- Session tree folders use a macOS-style `>` disclosure chevron (rotates when open).
- Left-click a folder icon/name expands or collapses children; right-click still opens the context menu (add server, rename, …).

### Fixed
- Folder name hover no longer shows the text I-beam cursor (arrow cursor instead).

### 中文
- **新增**：首次设置主密码时默认勾选屏幕锁；数据目录说明标明默认 `.vsterm`，路径可粘贴；首次安装默认启用底部小狗桌面宠物。
- **改进**：文件夹折叠箭头改为 macOS 风格 `>`；左键点名称展开/收起，右键可添加服务器等。
- **修复**：服务器列表里文件夹名悬停光标恢复为箭头。

## [1.1.5] — 2026-07-26

- CI: run release asset upload with bash on all runners (fixes Windows PowerShell failing on `set -euo pipefail`); bump actions/checkout to v5.

## [1.1.4] — 2026-07-26

- CI: tag releases upload assets directly to the public Release (skip Actions artifacts; avoids storage-quota failures).

## [1.1.3] — 2026-07-26

- Cut idle CPU: vsync on real GPUs (WARP still AutoNoVsync); stop tooltip popups from scheduling 60 fps redraws.
- macOS Dock / Launchpad icon: 10% transparent margin; runtime Dock glyph uses the same padded asset.
- macOS releases ship as drag-install DMGs; `tools/` (ip2region) lives inside `VsTerm.app/Contents/Resources`.

## [1.1.2] — 2026-07-24

- Verify private CI publishes Release assets (and docs) to public `vesaaa/vsterm`.

## [1.1.0] — 2026-07-24

- Fix terminal drag selection starting late (anchor at press origin so fast drags no longer skip the first characters).

## [1.0.49] — 2026-07-24

- SSH tunnels: one shared forward set per host across tabs; UI polish (icons, SOCKS5 defaults, panel height).

## [1.0.48] — 2026-07-23

- Six-locale UI, release polish, bilingual README.

## [1.0.47] — 2026-07-22

- Dual geo providers for path trace; network-connections filter refresh.

## [1.0.46] — 2026-07-21

- Commands panel: folders, icons, multi-line paste.

## Earlier

See [Releases](https://github.com/vesaaa/vsterm/releases) for the full history (`v1.0.0` … `v1.0.45`).
