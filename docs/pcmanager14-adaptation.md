# 电脑管家 14 本地适配测试版

日期：2026-11-26。目标安装包：`PCManager_Setup_14.0.7.310(C233)_x64.exe`。
状态：已实现安装器注入与运行补丁修正，进入手动安装、账号登录和手机协同验证阶段。
本说明不把源码编译通过等同于鸿蒙 7 兼容认证。

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
备份提交 `359b9be` 通过真实 Git 命令创建，并通过 `git fsck --full`。
更早的手工 Git 对象备份被 Git 检出损坏，本次使用独立源码仓库，未依赖该备份。

构建需要 VS 2022 C++ 工具、Windows SDK，以及以下两个 Rust 工具链：

```text
nightly-2025-02-01-x86_64-pc-windows-msvc
nightly-2025-02-01-i686-pc-windows-msvc
```

本机工具链安装在工作区 `work/build-tools`，没有修改用户的全局 PATH。
在源码目录执行 `build-release.bat`。输出仍为 x86 启动器和 x86 注入核心，
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
2. 点击“浏览”，选择 `C:\Users\hi\Downloads\PCManager_Setup_14.0.7.310(C233)_x64.exe`，再点击“安装”。如果安装器要求先卸载旧版本，记录提示并由用户手动处理。本次交付不自动卸载旧版本。
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
尚未证明 14 版在本机安装成功，也未证明鸿蒙 7 手机账号登录、USB 协同或无线投屏正常。
下一轮需要手机准确型号、完整系统版本，以及本测试版对应时间的新日志。

## 对照与恢复

本次没有覆盖原 D 盘启动器或已安装电脑管家，也没有改动代理、防火墙、证书或账号数据。
用户安装本测试版后如需回退，应退出相关进程，使用原启动器和原安装包按原方式重装；
旧版本是否保留手机协同能力取决于实际手机系统，不能作为保证。
保留本包、新旧日志和原安装包，避免把不同版本的 EXE、核心 DLL 与运行补丁混用。

参考源码：https://github.com/hamflx/huawei-pc-manager-bootstrap/pull/54
