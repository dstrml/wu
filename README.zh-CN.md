<p align="center"><a href="README.md">English</a> | 简体中文</p>

<div align="center">
  <img src="crates/wu/resources/app-icon.png" alt="Wu" width="128">
  <h1>Wu</h1>
  <p>快速的原生代码编辑器和智能体工作区。</p>
  <p>用 Rust 编写，开箱即用。</p>
  <p><a href="#下载"><strong>下载</strong></a></p>
</div>

---

Wu 是一款用 Rust 编写的快速原生代码编辑器，界面简洁现代，并内置了编程智能体。名字来自[无为](https://zh.wikipedia.org/wiki/无为)：不费力的行动。Wu 是 [Zed](https://github.com/zed-industries/zed) 的分支，但已经和它有了很大的不同。

## 功能

- ⚡ **原生、快速。** 不用 Electron，也没有网页视图。打开就能用，大型视图下也一直流畅。
- 🍃 **轻量。** Wu 占用的内存比 VS Code 和 Zed 都少。查看[测试结果](https://github.com/dstrml/wu/blob/main/docs/memory-benchmark.md)。
- 👾 **内置编程智能体。** 直接支持 Claude Code、Codex 和 OpenCode。
- ✨ **默认就好看。** 外观简洁现代，用起来也顺手。
- 🤫 **无需账号，没有遥测。** 不用登录任何东西。Wu 从不把你的使用数据发送到任何地方。

## 下载

在 macOS 和 Linux 上运行：

```sh
curl -fsSL https://wu.dstrml.org/install.sh | sh
```

Windows 用户请下载[安装程序](https://github.com/dstrml/wu/releases/latest/download/Wu-x86_64.exe)。

<details>
<summary>或者手动安装</summary>

- [macOS（Apple 芯片）](https://github.com/dstrml/wu/releases/latest/download/Wu-aarch64.dmg)
- [macOS（Intel 芯片）](https://github.com/dstrml/wu/releases/latest/download/Wu-x86_64.dmg)
- [Linux x86-64](https://github.com/dstrml/wu/releases/latest/download/wu-linux-x86_64.tar.gz)
- [Linux AArch64](https://github.com/dstrml/wu/releases/latest/download/wu-linux-aarch64.tar.gz)

更多内容请看 [INSTALL.md](https://github.com/dstrml/wu/blob/main/docs/INSTALL.md)。
</details>

---

![Wu 截图](https://wu.dstrml.org/screenshot.png)

## 文档

请看[文档](https://wu.dstrml.org/zh/)。

## 构建

Wu 的构建方式和 Zed 一样。请看 [Zed 开发文档](https://zed.dev/docs/development)。

## 许可证

Wu 采用 GPL-3.0-or-later 许可证，标注的部分采用 Apache-2.0。请看 [LICENSE-GPL](./LICENSE-GPL) 和 [LICENSE-APACHE](./LICENSE-APACHE)。

Wu 是 [Zed](https://github.com/zed-industries/zed) 的衍生作品，使用相同的许可证。

## 致谢

感谢 Zed 团队打造了这样出色的编辑器，并把它开源。没有他们的工作，就不会有 Wu。
