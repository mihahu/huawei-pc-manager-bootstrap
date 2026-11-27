# 华为电脑管家 14 安装辅助工具

基于 [hamflx/huawei-pc-manager-bootstrap](https://github.com/hamflx/huawei-pc-manager-bootstrap) 的社区适配版本，由 [mihahu](https://github.com/mihahu) 整理发布。本项目提供启动器和运行补丁，帮助在非华为 Windows 电脑上安装、运行电脑管家。

**2026-11-27 实测反馈：电脑管家 14.0.7.310 已安装，用户已使用星芒 O1 Pro 成功连接，反馈当前测试正常。** 手机系统此前由用户描述为鸿蒙 7；这是单台电脑与手机的使用反馈，不是所有鸿蒙 7 设备的兼容保证。

## 下载

- [发布页面与全部附件](https://github.com/mihahu/huawei-pc-manager-bootstrap/releases/tag/pcmanager14-test1)
- [下载 Windows 工具包](https://github.com/mihahu/huawei-pc-manager-bootstrap/releases/download/pcmanager14-test1/huawei-pc-manager-bootstrap-pcmanager14-test1-windows.zip)
- [下载配套源码](https://github.com/mihahu/huawei-pc-manager-bootstrap/releases/download/pcmanager14-test1/huawei-pc-manager-bootstrap-pcmanager14-test1-source.zip)
- [SHA-256 校验文件](https://github.com/mihahu/huawei-pc-manager-bootstrap/releases/download/pcmanager14-test1/SHA256SUMS.txt)

工具包内包含 `huawei-pc-manager-bootstrap.exe`、配套核心 DLL、使用说明和构建信息。华为电脑管家原版安装包与 HMS Core 由用户从华为官方渠道另行下载，未包含在本项目附件中。

## 使用

1. 从发布页面下载 Windows 工具包并完整解压，EXE 与同包 DLL 保持在同一目录。
2. 下载电脑管家 `PCManager_Setup_14.0.7.310(C233)_x64.exe`。可从[华为电脑管家官网](https://consumer.huawei.com/cn/support/pc-manager/)进入下载，核对实际版本。
3. 退出已有电脑管家及协同窗口，以管理员身份运行 `huawei-pc-manager-bootstrap.exe`。
4. 点击“浏览”选择电脑管家安装包，再点击“安装”，按安装界面提示完成操作。如提示先卸载旧版或重启，先完成该步骤，再重新运行启动器。
5. 安装后在电脑管家“关于”确认版本，分别测试账号登录和手机连接。如果提示“HMS Core 服务不支持”，先按提示下载并安装 Windows 版 HMS Core，然后退出托盘程序并重新打开。

完整操作、日志位置及已知限制见[适配说明](docs/pcmanager14-adaptation.md)。

## 本版修改与验证

本版基于 `wornik` 的 `v0.1.13-fix`，保留其 DLL 固定修复，补充初始化失败处理、注入诊断、固件缓冲区边界处理和有边界检查的 SMBIOS 改写。原 UUID、原始句柄及其他固件记录保留，固件返回值只影响加载补丁的进程，不写入实际 BIOS。

公共模块在 x64/x86 各通过 10 项测试，启动器通过 35 项测试，共 55 次测试执行、45 项不同测试。真实 DLL 加载、系统版本接口转发、固件查询、子进程注入及模块释放后的回调验证均通过。

发布二进制沿用用户已测试的 `0.1.9-883d64a`，代码基线为 `883d64aa1fd02c229521abc254bfcf3a506077ee`。`pcmanager14-test1` 是本分支发布标识；上游 Cargo 包版本仍为 `0.1.9`。源码标签还包含后续发布说明和 CI 触发配置。

x86 安装核心对部分 x64 辅助进程会记录 `ArchMismatch`，本机后续安装和连接成功，但其他硬件、HMS Core 版本、完整系统构建号及各连接方式仍需分别测试。本项目为社区工具，不代表华为官方支持。

## 从源码构建

需要 Windows x64、Visual Studio 2022 C++ 构建工具、Windows SDK 和 Rustup。

```powershell
git clone --branch pcmanager14-adaptation https://github.com/mihahu/huawei-pc-manager-bootstrap.git
cd huawei-pc-manager-bootstrap
rustup toolchain install nightly-2025-02-01-x86_64-pc-windows-msvc --profile minimal
rustup toolchain install nightly-2025-02-01-i686-pc-windows-msvc --profile minimal --force-non-host
.\build-release.bat
```

输出位于 `dist`。启动器与核心为 x86；先行构建的 x64 `version.dll` 嵌入启动器。
测试与探针命令见[适配说明](docs/pcmanager14-adaptation.md#自动验证)。

GitHub Actions 对 `master` 与 `pcmanager14-adaptation` 的推送、PR 执行构建验证。此次 `pcmanager14-test1` 发布使用已实测二进制，未使用上游 `v*` 标签自动发布流程。

## 来源与致谢

感谢 **[hamflx](https://github.com/hamflx)** 创建原项目并公开安装检查处理、进程注入和运行补丁实现；感谢 **[wornik](https://github.com/wornik)** 在 [PR #54](https://github.com/hamflx/huawei-pc-manager-bootstrap/pull/54) 及 [v0.1.13-fix](https://github.com/wornik/huawei-pc-manager-bootstrap/releases/tag/v0.1.13-fix) 中修复注入核心 DLL 提前释放问题。

本分支是基于上述成果的修改与验证，不将原项目或他人修复宣称为原创。完整来源见 [CREDITS.md](CREDITS.md)；本地适配前的项目 README 保留在 [docs/README-upstream.md](docs/README-upstream.md)，其中版本和下载地址属于原项目记录。
