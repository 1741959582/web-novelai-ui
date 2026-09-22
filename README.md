# web-novelai-ui

中文 NovelAI 图像创作桌面端。左上角菜单进入各功能。下面按第一次打开的顺序说明，每项都有界面截图。

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

WD 走公开空间，一般不用额外填写。CL 若已在设置里启用本地模型，就用本机显卡；否则走 Hugging Face 空间。匿名额度用完后，到「设置 → 本地打标」填 Hugging Face read token 再试。

![反推页：左侧选模型和阈值，右侧输出提示词](docs/assets/guide/reverse.jpg)

## APNG

把真图藏进动画，聊天里先看到封面，点开才是原图。也可以合成 GIF、还原、清元数据、打马赛克。

1. 选「内置封面」或「自定义封面」。
2. 「加真图」或从生成页粘贴。加入时会清掉 NovelAI 元数据。
3. 左边是对方看到的封面，右边是点开后的画面。
4. 点「伪装当前」或「全部伪装」，再用「复制图片」贴到聊天窗口。

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

![设置：本地打标](docs/assets/guide/settings-tagger.jpg)

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
| 反推没有标签 | 看是 Hugging Face 额度用完，还是本地模型没下载 |

## 开发

与 [Tauri v2 Windows 前置要求](https://v2.tauri.app/zh-cn/start/prerequisites/) 一致：Visual Studio C++ 桌面开发、WebView2、`rustup default stable-msvc`、Node.js ≥ 18。

```bash
npm install
npm run dev
```

打包：`npm run build`（NSIS，见 `src-tauri/tauri.conf.json`）。
