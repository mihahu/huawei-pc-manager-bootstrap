# 来源与致谢

本仓库由 [mihahu](https://github.com/mihahu) 整理和发布电脑管家 14 社区适配版本。原项目、已有修复及第三方依赖的作者归属保持不变。

| 来源 | 作者 | 本分支使用的成果 |
| --- | --- | --- |
| [huawei-pc-manager-bootstrap](https://github.com/hamflx/huawei-pc-manager-bootstrap) | [hamflx](https://github.com/hamflx) 及原项目贡献者 | 安装辅助工具、安装检查处理、进程注入、固件查询挂钩和运行代理 DLL 的基础实现 |
| [PR #54](https://github.com/hamflx/huawei-pc-manager-bootstrap/pull/54) | [wornik](https://github.com/wornik) | 固定注入核心 DLL，修复注入句柄释放后模块提前卸载问题 |
| [v0.1.13-fix](https://github.com/wornik/huawei-pc-manager-bootstrap/releases/tag/v0.1.13-fix) | [wornik](https://github.com/wornik) | 本地适配使用的源码基线，提交 `99f3574ffa3deaa49f44d1982f4fff6f926c6292` |
| [原理分析文章](https://bbs.pediy.com/thread-270682.htm) | 原文作者，原项目已引用 | 原项目提供的背景参考，未在本次发布中重新核验全文 |

感谢以上作者公开技术实现和修复，也感谢 Rust、Windows API 绑定、`injectors`、`retour`、`forward-dll` 等依赖项目的贡献者。实际依赖版本以 `Cargo.lock` 为准，各组件保留自身的作者和许可信息。

本次本地修改包括 DLL 初始化和日志处理、有边界检查的 SMBIOS 解析、固件 API 缓冲区处理、SKU 引用修复、回归测试及真实进程探针。发布二进制代码提交为 `883d64aa1fd02c229521abc254bfcf3a506077ee`；后续提交记录发布文档和测试反馈。

源码从上述发布快照建立本地 Git 仓库，早期提交并不包含原项目的完整历史；原始提交及贡献记录请访问对应上游仓库。保留的[适配前 README](docs/README-upstream.md)用于追溯来源，其中产品介绍、链接与版本结论不作为本分支的新承诺。

华为电脑管家、HMS Core 及相关商标归各自权利人所有。本发布仅分发该社区工具的源码与自编译产物，不附带华为原版安装包。
