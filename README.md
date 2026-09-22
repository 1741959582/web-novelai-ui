# web-novelai-ui

中文 NovelAI 图像创作桌面端。左上角菜单进入各功能。下面按第一次打开的顺序说明，每项都有界面截图。

## 下载与安装

**[GitHub 最新发行版](https://github.com/caiweida/web-novelai-ui/releases/latest)**

以下直链对应 **[v0.1.4](https://github.com/caiweida/web-novelai-ui/releases/tag/v0.1.4)**；后续版本请使用上面的「最新发行版」。

| 平台 | 选择安装包 | 使用说明 |
| --- | --- | --- |
| Windows x64 · 安装版 | [Setup.exe](https://github.com/caiweida/web-novelai-ui/releases/download/v0.1.4/NAI-Studio-Web-UI-Setup-0.1.4.exe) | 长期使用建议选这个；安装向导、开始菜单快捷方式 |
| Windows x64 · 便携版 | [便携版.zip](https://github.com/caiweida/web-novelai-ui/releases/download/v0.1.4/NAI-Studio-Web-UI-0.1.4-portable.zip) | 解压后双击 `web-novelai-ui.exe`；更新时下载新包替换整个文件夹 |

需要 Windows 10 及以上。系统没有 WebView2 时，安装版会提示安装。第一次打开按向导填写 NovelAI Token。设置和生成的图片保存在本机，不在安装包旁边。启动后会读取 GitHub Release，有新版本时顶栏可以一键下载安装包。

## 使用流程

1. 第一次打开，按向导填写 NovelAI Token。本地打标可以跳过。
2. 打开左上角菜单，进入「生成」，写提示词，点 Generate。
3. 图片出现在中间预览，并保存到输出目录。右侧 History 可以回看。
4. 需要多张排队时用「批量」，要从图得到标签时用「反推」，要藏图或清元数据时用「APNG」。

只有 Token 是必填。输出目录、代理、标签库、本地打标都可以之后再改。

## 第一次打开

启动后弹出三步向导。

**第 1 步：填写 NovelAI Token。** 粘贴 `pst-` 开头的 Persistent API Token，点「验证」。也可以先点「下一步」，之后到「设置 → API 配置」再填。不填不能生图。

![首次向导第 1 步，填写 NovelAI Token](docs/assets/guide/onboarding-1.jpg)

**第 2 步：本地打标，可以跳过。** 程序会检测 NVIDIA 显卡。没有合适显卡时保持关闭，反推仍可用 WD Tagger 或 Hugging Face 上的 CL Tagger。

![首次向导第 2 步，本地打标可以跳过](docs/assets/guide/onboarding-2.jpg)

**第 3 步：进入应用。** 点「进入应用」后，下次启动不再弹出向导。

![首次向导第 3 步，进入应用](docs/assets/guide/onboarding-3.jpg)

Token 只保存在本机，不要贴到截图或聊天里。

## 从菜单进入功能

点左上角三条横线。当前可用入口如下。

![功能菜单](docs/assets/guide/menu.jpg)

## 生成

写提示词、选模型、加角色，然后点 **Generate**。

1. 左侧选模型。角色提示词只在 V4 及以上可用。
2. Prompt 写想要的内容，Undesired Content 写不想要的内容。
3. 点角色卡片右上角加号，给每个角色单独写提示词。
4. 中间 Get Started 里的示例图可以点一下，把提示词抄进输入框。
5. 张数先用 1，确认尺寸后再生成。结果在中间预览，并写入输出目录。右侧 History 可回看。

```text
1girl, solo, blue hair, blue eyes, white dress, garden, sunlight, smile
```

![生成页：左侧提示词和角色，中间示例图](docs/assets/guide/generate.jpg)

## 批量

把多条提示词排成队，按当前生成页的模型、尺寸和采样器依次生成。

1. 左边一行写一条主提示词。
2. 要用不同角色时，用 `---` 分段，并写 `角色:`、`负面:`、`尺寸:`。
3. 下面的角色卡片和生成页一样，点加号新增。这些角色会写进每条新任务。
4. 点「加入队列」，再点「开始排队生成」。

![批量生成：左侧录入，右侧队列](docs/assets/guide/batch.jpg)

## 反推

从图片得到 Danbooru 标签，再应用到生成页。

1. 拖入图片，或点「使用当前生成图」。
2. 模型选 WD Tagger，或 CL Tagger v2。
3. 点「开始反推」。完成后在右侧复制，或点「应用到生成」。

WD 走公开空间，一般不用额外填写。CL 若已在设置里启用本地模型，就用本机显卡；否则走 Hugging Face 空间。匿名额度用完后，到「设置 → 本地打标」按下面的步骤填写 Hugging Face read token。

![反推页：左侧选模型和阈值，右侧输出提示词](docs/assets/guide/reverse.jpg)

## APNG

把真图藏进动画，聊天里先看到封面，点开才是原图。也可以合成 GIF、还原、清元数据、打马赛克。

1. 选「内置封面」或「自定义封面」。
2. 「加真图」或从生成页粘贴。加入时会清掉 NovelAI 元数据。
3. 左边是对方看到的封面，右边是点开后的画面。
4. 点「伪装当前」或「全部伪装」，再用「复制图片」或在预览上右键，粘贴到聊天窗口。复制的是 APNG 文件本身，不是预览位图。

![APNG 伪装：封面预览和点开效果](docs/assets/guide/apng.jpg)

## 参考预设

管理本地参考图预设，也可以读取在线目录。滚到列表底部会继续加载下一页。

![参考预设](docs/assets/guide/reference.jpg)

## 法典

按章节和分类查找提示词。先选一部法典，再搜索标题或提示词。滚到底会拼接下一页。

![法典图鉴](docs/assets/guide/gallery.jpg)

## 原数据

打开一张带 NovelAI 信息的 PNG，查看提示词、模型、种子和尺寸，再套用回生成页。

![原数据](docs/assets/guide/metadata.jpg)

## 工具

工具页汇总批量、反推、APNG 等入口。点有路径的卡片会进入对应功能。

![工具页](docs/assets/guide/tools.jpg)

## 记录

已经生成并写入历史的图片会列在这里，可以套用参数或打开输出目录。还没生成过时，这里是空的。

![记录](docs/assets/guide/records.jpg)

## 后期、酒馆

菜单里有这两项，页面还是占位，云端超分、Director Tools 和酒馆对话生图尚未接入。

![后期占位页](docs/assets/guide/postprocess.jpg)

![酒馆占位页](docs/assets/guide/tavern.jpg)

## 设置

左上角方框按钮，或菜单里的「设置」。**只有 API Token 必须填**，其余保持默认即可。

### API 配置

粘贴 Token，点「验证 Token / 刷新积分」。能看到订阅档位和 Anlas 就说明可用。

![设置：API 配置](docs/assets/guide/settings-api.jpg)

### 存储与网络

输出目录留空，图片保存到「图片 / Langbai NovelAI」。Image API 保持官方地址。代理留空就是直连，官方接口打不开时再填，例如 `http://127.0.0.1:7890`。用官方接口时，不要勾选非官方端点。改完要点「保存设置」。

![设置：存储与网络](docs/assets/guide/settings-storage.jpg)

### 提示词补全

不下载就用内置精简词库。下载后可以用中文或英文补全，并看到热度。

![设置：提示词补全](docs/assets/guide/settings-tags.jpg)

### 本地打标

可选。需要 NVIDIA 显卡，权重另外下载，不打包在程序里。不启用也不影响生图和 WD 反推。

下载权重需要 Hugging Face token，而且必须先用**同一个账号**在模型页同意许可。Token 只保存在本机，不要贴到截图或聊天里。输入框旁的 **?** 移入鼠标，可以看到同样的步骤。

![鼠标移到问号上，显示 token 教程](docs/assets/guide/hf-token-help.jpg)

**1. 同意许可。** 打开 [cella110n/cl_tagger_v2](https://huggingface.co/cella110n/cl_tagger_v2)，登录后点 Agree。页面上会写要分享联系方式才能访问文件。没点过 Agree，下载会返回 403。

![模型页：登录后同意许可才能下载权重](docs/assets/guide/hf-model-top.jpg)

**2. 创建 Read token。** 打开 [Access Tokens](https://huggingface.co/settings/tokens)，点 **New token**。名字随便写，Role 选 **read**，再点 **Generate a token**。复制 `hf_` 开头的那一串。

![Access Tokens 页面，点 New token](docs/assets/guide/hf-token-list.jpg)

![新建 token 时 Role 选择 read](docs/assets/guide/hf-token-create.jpg)

如果页面上只有 Fine-grained、没有 read：勾选读取你已经同意的公开 gated 仓库，或只给 `cella110n/cl_tagger_v2` 读取权限。Write token 也能下载，但下载不需要写权限。

**3. 填回本地打标。** 「设置 → 本地打标」，把 token 粘贴到 Hugging Face token，勾选「启用本地 CL Tagger v2」，点「下载模型到本机」。`model.onnx.data` 大约 2.2GB，进度显示在按钮上面。连不上 huggingface.co 时会自动改试 hf-mirror.com；仍失败就在同一页填写代理。

![设置：本地打标，问号里是同一份 token 教程](docs/assets/guide/settings-tagger.jpg)

### 性能

流式预览默认开着，生成过程会逐步显示。失败会自动改用整张结果。生成页右上角也能开关。

![设置：性能](docs/assets/guide/settings-perf.jpg)

参考桌面版里的「AI 反推视觉接口」「转换 API」「百度翻译密钥」这里没有对应设置，不用去配。反推用本页的 WD / CL，翻译用内置功能。

## 连不上时

| 现象 | 先看 |
| --- | --- |
| Token 验证失败或 401 | Token 是否完整、是不是 `pst-` 开头。不要粘贴浏览器 Cookie |
| 余额或模型不可用 | 账号权限、Anlas，以及当前尺寸和张数 |
| 超时、连接重置 | 先试直连；仍失败再在「存储与网络」填代理 |
| 本地模型下载超时或缺文件 | 安装版会自动改试 hf-mirror.com。仍失败就在本地打标页填写代理，例如 `http://127.0.0.1:7890`。中断后可再点下载续传 |
| 反推没有标签 | 看是 Hugging Face 额度用完，还是本地模型没下载 |
| 本地模型下载 403 | 创建 token 的账号要在模型页点过 Agree，Role 用 read。步骤见上面「本地打标」 |

## 开发

与 [Tauri v2 Windows 前置要求](https://v2.tauri.app/zh-cn/start/prerequisites/) 一致：Visual Studio C++ 桌面开发、WebView2、`rustup default stable-msvc`、Node.js ≥ 18。

```bash
npm install
npm run dev
```

打包：`npm run build`（NSIS，见 `src-tauri/tauri.conf.json`）。
