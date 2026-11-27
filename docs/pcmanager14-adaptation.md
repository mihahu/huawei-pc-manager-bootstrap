# 电脑管家 14 本地适配测试版

更新日期：2026-11-27。目标安装包：`PCManager_Setup_14.0.7.310(C233)_x64.exe`。
状态：安装器注入与运行补丁修正已完成；用户已安装电脑管家 14，并反馈星芒 O1 Pro 已成功连接、目前测试正常。
这是单机实测反馈，不等同于全部鸿蒙 7 设备、连接方式和功能均兼容。

## 已验证状态

- 2026-11-26：公共模块在 x64/x86 各通过 10 项测试，启动器通过 35 项测试，共 55 次测试执行、45 项不同测试。
- x64 运行补丁的系统版本接口转发、真实固件查询、缓冲区边界和模块生命周期验证通过。
- x86 核心和真实 `CreateProcessW` 子进程注入验证通过。
- 2026-11-27：本机卸载信息显示电脑管家 `14.0.7.310`，运行日志确认自编译运行补丁已加载。
- 2026-11-27：用户反馈“目前测试没有问题，我已经使用星芒 O1 Pro 连上了”。手机此前由用户描述为鸿蒙 7；完整系统构建号、连接方式及每项协同功能尚未单独记录。

本次公开包沿用用户已测试的 `0.1.9-883d64a` 二进制，不因整理发布文档重新编译。
源码标签包含后续发布文档；实际二进制代码基线为 `883d64aa1fd02c229521abc254bfcf3a506077ee`。

## 修改范围

- 保留上游 PR #54 的 DLL 固定修复，并在固定失败时停止安装挂钩；先初始化日志，避免遗漏固定失败的原因。
- 运行补丁也固定自身模块，返回实际初始化失败，避免错误记录为成功。
- 仅在安装器要求注入子进程时初始化 `CreateProcessW` 挂钩，运行补丁只处理固件查询。
- 固件查询遵循大小查询、短缓冲区和较大缓冲区行为，修复较大缓冲区复制可能触发的 panic。
- 用有边界检查的 SMBIOS 解析替代裸指针遍历，更新原始表的长度字段；保留 UUID、原始句柄、字符串及其他记录，只改写 Type 1 的指定字符串引用。
- 修复 SKU 字符串引用被清零的问题；不再把序列号写进固件诊断日志。
- 注入失败日志记录错误枚举详情。依赖库把位数不匹配也显示为 `Bincode Error`，本版可区分该情况。

以上固件信息仅在加载补丁的进程内返回，不写入实际 BIOS。

## 构建基线

源码基于 `wornik/huawei-pc-manager-bootstrap` 的 `v0.1.13-fix`，
上游提交 `99f3574ffa3deaa49f44d1982f4fff6f926c6292`。
本地适配提交 `883d64a`；修改前已建立 Git 备份，源码仓库通过 `git fsck --full`。
项目及作者来源见 [来源与致谢](../CREDITS.md)。

构建需要 VS 2022 C++ 工具、Windows SDK，以及以下两个 Rust 工具链：

```text
nightly-2025-02-01-x86_64-pc-windows-msvc
nightly-2025-02-01-i686-pc-windows-msvc
```

安装工具链后，在源码目录执行 `build-release.bat`。输出仍为 x86 启动器和 x86 注入核心，
启动器嵌入先行构建的 x64 `version.dll`，供 x64 电脑管家使用。
上游 Cargo 包版本仍为 `0.1.9`；窗口中的 Git 提交后缀及交付清单标识本地构建，
不能仅凭窗口里的基础版本号判断使用的是哪个发布包。

## 自动验证

以下命令均不运行华为安装包：

```powershell
cargo +nightly-2025-02-01-x86_64-pc-windows-msvc test --locked --release -p common --target=x86_64-pc-windows-msvc
cargo +nightly-2025-02-01-i686-pc-windows-msvc test --locked --release -p common -p huawei-pc-manager-bootstrap --target=i686-pc-windows-msvc
cargo +nightly-2025-02-01-x86_64-pc-windows-msvc build --locked --release -p common --example firmware_probe --target=x86_64-pc-windows-msvc
cargo +nightly-2025-02-01-i686-pc-windows-msvc build --locked --release -p common --example firmware_probe --target=i686-pc-windows-msvc
```

在独立进程中验证真正的 Windows 固件 API 和已构建的 DLL：

```powershell
& .\target\x86_64-pc-windows-msvc\release\examples\firmware_probe.exe runtime "$PWD\target\x86_64-pc-windows-msvc\release\version.dll"
& .\target\i686-pc-windows-msvc\release\examples\firmware_probe.exe core "$PWD\target\i686-pc-windows-msvc\release\huawei_pc_manager_bootstrap_core.dll"
& .\target\i686-pc-windows-msvc\release\examples\firmware_probe.exe spawn "$PWD\target\i686-pc-windows-msvc\release\huawei_pc_manager_bootstrap_core.dll"
```

`runtime` 验证 x64 代理 DLL 的固件修改及系统版本接口转发；
`core` 验证 x86 核心导出与模块固定；
`spawn` 验证真实 `CreateProcessW` 子进程注入、参数传递及 `InjectionGuard` 释放后的回调存活。
探针不写入华为安装目录，不启动真实电脑管家，也不验证账号服务器或手机连接。

## 手动安装与验收

1. 退出电脑管家及手机协同窗口。在交付目录中启动新编译的 `huawei-pc-manager-bootstrap.exe`，按需接受管理员权限提示。EXE 与同包核心 DLL 必须放在一起，避免与旧发布包混用。
2. 从华为官网下载电脑管家安装包，点击“浏览”选择下载的 `PCManager_Setup_14.0.7.310(C233)_x64.exe`，再点击“安装”。如果安装器要求先卸载旧版本或重启，先按提示完成后再以管理员身份运行启动器。
3. 安装完成后打开电脑管家“关于”，确认实际版本为 `14.0.7.310`。仅安装器结束或已有 `PCManager.exe` 不足以证明升级成功。
4. 单独测试一次账号登录，记录准确时间及完整提示。再测试手机发现、有线连接、无线连接和投屏。每项分别记录结果，避免把安装成功当作协同成功。
5. 出现错误时记录时间，优先核对下面的日志。提供日志前去除账号、令牌、二维码、序列号等个人信息。

日志位置：

```text
%LOCALAPPDATA%\hamflx\huawei_pc_manager_bootstrap\cache
C:\ProgramData\Comms\PCManager\log
C:\ProgramData\Huawei\HMSCORE\log
```

若提示补丁文件正在使用，先退出电脑管家及协同进程，再重试；不要把文件锁错误当作安装成功。
如果提示“HMS Core 服务不支持”，先按电脑管家的“下载并安装”提示补齐或修复 Windows 版 HMS Core，再退出托盘程序并重新打开测试。
该提示是本机安装过程中曾出现的现象；未单独记录最终 HMS Core 版本及处理步骤，不能声称本工具已修改 HMS Core。

安装日志可能出现 `ArchMismatch("Expect x64", "Got x86")`：
x86 注入核心不能直接注入 x64 安装辅助进程。本机后续已成功安装和连接，
但这不代表所有安装分支均被覆盖；其他机器失败时应保留对应日志继续定位。

## 对照与恢复

构建与自动验证不运行华为安装程序；用户手动安装会写入电脑管家和运行补丁。
本工具不修改实际 BIOS；本次适配未改动代理、防火墙、证书或账号数据。
安装本测试版后如需回退，应退出相关进程，使用原启动器和原安装包按原方式重装；
旧版本是否保留手机协同能力取决于实际手机系统，不能作为保证。
保留本包、新旧日志和原安装包，避免把不同版本的 EXE、核心 DLL 与运行补丁混用。

参考源码：https://github.com/hamflx/huawei-pc-manager-bootstrap/pull/54
