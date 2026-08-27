import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { findUntranslatedStaticCopy, svelteFilesUnder } from "../../scripts/i18n-audit";
import {
  LANGUAGE_OPTIONS,
  normalizeLanguage,
  translateText,
} from "./i18n";

function expectTranslations(cases: ReadonlyArray<readonly [string, string]>): void {
  for (const [source, expected] of cases) {
    expect(translateText(source, "zh-CN"), source).toBe(expected);
  }
}

describe("Cortex UI localization", () => {
  test("accepts the supported English and Simplified Chinese language codes", () => {
    expect(normalizeLanguage("en")).toBe("en");
    expect(normalizeLanguage("zh-CN")).toBe("zh-CN");
    expect(normalizeLanguage("zh")).toBe("zh-CN");
    expect(normalizeLanguage("unsupported")).toBe("en");
    expect(LANGUAGE_OPTIONS).toEqual([
      { id: "zh-CN", label: "中文" },
      { id: "en", label: "English" },
    ]);
  });

  test("translates known interface copy while preserving surrounding whitespace", () => {
    expect(translateText("Settings", "zh-CN")).toBe("设置");
    expect(translateText("  Your subjects\n", "zh-CN")).toBe("  我的课程\n");
  });

  test("translates dynamic counts and labels without changing user content", () => {
    expect(translateText("3 sources", "zh-CN")).toBe("3 个来源");
    expect(translateText("Add topic to Calculus", "zh-CN")).toBe("向 Calculus 添加主题");
    expect(translateText("My Calculus Notes", "zh-CN")).toBe("My Calculus Notes");
  });

  test("English mode always returns the original source copy", () => {
    expect(translateText("Settings", "en")).toBe("Settings");
    expect(translateText("3 sources", "en")).toBe("3 sources");
  });

  test("marks user-authored picker and subject labels as outside the DOM translation boundary", () => {
    const picker = readFileSync("src/components/Picker.svelte", "utf8");
    expect(picker).toContain("userContent?: boolean");
    expect(picker.match(/data-i18n-skip=\{(?:cur\?\.|o\.)userContent \|\| undefined\}/g)?.length).toBe(2);

    const subjectView = readFileSync("src/views/SubjectView.svelte", "utf8");
    expect(subjectView).toContain('<span data-i18n-skip>{scopeTopicName}</span>');
    expect(subjectView).toContain('{translateText("Whole subject", app.language)}');

    const analyticsView = readFileSync("src/views/AnalyticsView.svelte", "utf8");
    expect(analyticsView).toContain('class="an-radar-lbl" text-anchor="middle" data-i18n-skip');
    expect(analyticsView).toContain('<title data-i18n-skip>{radarTopicSummary(t)}</title>');
    expect(analyticsView).toContain('class="an-name read" data-i18n-skip');
    expect(analyticsView).toContain('class="an-weak-subj" data-i18n-skip');
    expect(analyticsView).toContain('<span data-i18n-skip>{t.topic_name}</span>');
  });

  test("translates supported settings and creation copy in Chinese mode", () => {
    const cases: Array<[string, string]> = [
      // Create subject: headings, dynamic preview fallbacks, labels and placeholders.
      ["holds topics, sources and one living cheatsheet", "包含主题、来源和一份持续更新的速查表"],
      ["Untitled subject", "未命名课程"],
      ["no code", "无课程代码"],
      ["topics", "个主题"],
      ["SUBJECT NAME", "课程名称"],
      ["e.g. Algorithms", "例如：算法"],
      ["COURSE CODE", "课程代码"],
      ["optional", "可选"],
      ["COLOR", "颜色"],
      ["GLYPH", "图标"],
      ["FIRST TOPICS", "初始主题"],
      ["optional — add lectures into these", "可选 — 可将课程内容添加到这些主题中"],
      ["Recursion", "递归"],
      ["Dynamic programming", "动态规划"],
      ["Graphs", "图论"],
      ["Create subject", "创建课程"],

      // Model routing and API-key settings.
      ["Chat", "对话"],
      ["Scoped Q&A across sources", "基于来源的上下文问答"],
      ["Cheatsheet synthesis", "速查表整理"],
      ["Completeness-checked merges", "经过完整性检查的内容合并"],
      ["Audio overview script", "音频概览脚本"],
      ["Two-host podcast dialogue", "双主持人播客对话"],
      ["Quiz generation", "测验生成"],
      ["MCQ · short answer · cloze", "选择题 · 简答题 · 完形填空"],
      ["Flashcard generation", "闪卡生成"],
      ["Q/A pairs + SRS scheduling", "问答卡片 + 间隔重复安排"],
      ["Embedding", "嵌入模型"],
      ["Vector index for retrieval", "用于检索的向量索引"],
      ["n/a", "不适用"],
      ["Ollama tasks run fully offline on this machine or your homelab — no key required.", "Ollama 任务可在本机或家庭服务器上完全离线运行，无需密钥。"],
      ["Custom endpoint", "自定义接口"],
      ["Custom endpoint URL", "自定义接口地址"],
      ["OpenAI-compatible base URL", "兼容 OpenAI 的基础地址"],
      ["Custom endpoint API key", "自定义接口 API 密钥"],
      ["Bearer token for the custom endpoint", "自定义接口的 Bearer 令牌"],

      // Keybind settings.
      ["Starting point for bindings.", "快捷键绑定的起始预设。"],
      ["Command palette", "命令面板"],
      ["Leader menu", "引导键菜单"],
      ["Toggle chat", "显示或隐藏对话"],
      ["Toggle sidebar", "显示或隐藏侧边栏"],
      ["Cycle theme", "切换主题"],
      ["Music panel", "音乐面板"],
      ["Insert / focus compose", "输入 / 聚焦编辑框"],
      ["Go to dashboard (after g)", "前往主页（先按 g）"],
      ["Help overlay", "帮助面板"],

      // Integrations. Some sentences are deliberately split by inline code spans.
      ["Everything works out of the box on this computer. Add a cloud API key for heavier lifting, or point Cortex at your own homelab (the", "本机开箱即用。需要更强能力时，可以添加云端 API 密钥，或将 Cortex 连接到自己的家庭服务器（使用"],
      ["docker compose) — nothing here is required.", "Docker Compose）；这里的配置都不是必需的。"],
      ["Runs on the WhisperX lecture server behind your Homelab URL (configured below) — built for hour-plus recordings, with speaker labels when the server has an HF token. Audio never leaves your machines.", "通过下方配置的家庭服务器地址使用 WhisperX 课程转写服务，适合一小时以上的录音；服务器配置 HF 令牌后还可识别说话人。音频始终保留在你的设备中。"],
      ["legacy servers only", "仅旧版服务器"],
      ["Only used by OpenAI-compatible servers (speaches). The WhisperX lecture server picks its model in", "仅用于兼容 OpenAI 的服务器（如 speaches）。WhisperX 课程转写服务会在"],
      ["instead (", "中选择模型（"],
      [", default", "，默认"],
      [") — leave this blank there.", "）；使用该服务时请将此处留空。"],
      ["One address for everything. Run the", "一个地址连接所有服务。运行"],
      ["docker compose and point Cortex here — search, lecture transcription, instant sync, mobile ingest and local models are all reached off this single URL (Cortex adds", "Docker Compose 并让 Cortex 连接到这里；搜索、课程转写、即时同步、移动端导入和本地模型都通过这一个地址访问（Cortex 会自动添加"],
      ["for you). Add a Tailscale and/or public address and Cortex auto-picks the first reachable:", "等路径）。还可添加 Tailscale 和/或公网地址，Cortex 会自动选择第一个可访问的地址："],
      ["Your homelab's LAN address (the Caddy proxy port, default", "家庭服务器的局域网地址（Caddy 代理端口，默认"],

      // Calendar, experimental, audio and about settings.
      ["Not configured", "尚未配置"],
      ["Credentials", "凭据"],
      ["Create an OAuth client of type “Desktop app” in Google Cloud → APIs & Services → Credentials, enable the Calendar API, then paste the ID and secret here.", "在 Google Cloud → API 和服务 → 凭据中创建“桌面应用”类型的 OAuth 客户端，启用 Calendar API，然后在此粘贴客户端 ID 和密钥。"],
      ["Client ID", "客户端 ID"],
      ["Client secret", "客户端密钥"],
      ["Pull grades, assignments, deadlines and announcements from your Moodle portal into Cortex.", "将 Moodle 门户中的成绩、作业、截止日期和公告同步到 Cortex。"],
      ["YouTube streaming", "YouTube 串流"],
      ["Paste a YouTube video or livestream URL in the music panel to stream it ad-free. Uses a headless", "在音乐面板中粘贴 YouTube 视频或直播地址，即可无广告播放。此功能使用无界面的"],
      ["(auto-downloaded on first use). Nothing is bundled — only the URL is saved.", "（首次使用时自动下载）。应用不会捆绑这些工具，只保存视频地址。"],
      ["Tools", "工具"],
      ["Install mpv to enable YouTube streaming: sudo pacman -S mpv", "安装 mpv 以启用 YouTube 串流：sudo pacman -S mpv"],
      ["Focus timer", "专注计时器"],
      ["Pomodoro session lengths (applies app-wide).", "设置番茄钟时长（全局生效）。"],
      ["Focus length", "专注时长"],
      ["Short break", "短休息"],
      ["Long break", "长休息"],
      ["Sessions / set", "每组专注次数"],
      ["min", "分钟"],
      ["Cortex is built by one student. If it saves you time, a coffee keeps the updates coming.", "Cortex 由一名学生独立开发。如果它帮你节省了时间，一杯咖啡可以支持项目持续更新。"],
      ["Support me on Ko-fi", "在 Ko-fi 上支持我"],
      ["Engine", "技术栈"],
      ["Theme source", "主题来源"],
      ["Source-available · BYOK", "源码可用 · 自带密钥"],
      ["Offline-first. Your notes never leave this machine unless you choose a cloud model.", "离线优先。除非你选择云端模型，否则笔记不会离开本机。"],
    ];

    expectTranslations(cases);
  });

  test("keeps provider, model and command identifiers unchanged", () => {
    expect(translateText("OpenRouter", "zh-CN")).toBe("OpenRouter");
    expect(translateText("DeepSeek V4 Flash", "zh-CN")).toBe("DeepSeek V4 Flash");
    expect(translateText("docker-compose", "zh-CN")).toBe("docker-compose");
  });

  test("translates dynamic settings labels, statuses and feedback", () => {
    const cases: Array<[string, string]> = [
      ["Find on page", "在页面中查找"],
      ["Close overlay / go back", "关闭浮层 / 返回"],
      ["Go to dashboard", "前往主页"],
      ["Jump to subject N", "跳转到第 N 门课程"],
      ["Ollama (local)", "Ollama（本地）"],
      ["Gemini 2.5 Flash — ⚡ best value", "Gemini 2.5 Flash — ⚡ 最佳性价比"],
      ["GPT-5 mini — cheap + smart", "GPT-5 mini — 经济且智能"],
      ["Claude Sonnet 4.6 — ★ premium", "Claude Sonnet 4.6 — ★ 高端"],
      ["Mistral Small — light, local", "Mistral Small — 轻量本地"],
      ["Keybind updated", "快捷键已更新"],
      ["Key already in use", "按键已被使用"],
      ["Checking the server (a first-time model download can take a few minutes)…", "正在检查服务器（首次下载模型可能需要几分钟）…"],
      ["Synced", "已同步"],
      ["Sync error", "同步错误"],
      ["Moodle connected", "Moodle 已连接"],
      ["Moodle sync failed", "Moodle 同步失败"],
      ["Google Calendar connected", "Google 日历已连接"],
      ["Storage optimized", "存储已优化"],
      ["Reclaimed unused space (VACUUM).", "已回收未使用的空间（VACUUM）。"],
      ["Keys saved", "密钥已保存"],
      ["Stored in the system keychain.", "已保存到系统钥匙串。"],
      ["OpenRouter models", "OpenRouter 模型"],
      ["Couldn’t load the model list — check your connection.", "无法加载模型列表，请检查网络连接。"],
    ];

    expectTranslations(cases);
  });

  test("translates recorder states and actionable microphone permission guidance", () => {
    const cases: Array<[string, string]> = [
      ["READY", "就绪"],
      ["Press", "按下"],
      ["or click to start · output becomes a transcribed source", "或点击开始 · 录音会转写并保存为资料来源"],
      ["Microphone permission is off. Allow Cortex in System Settings → Privacy & Security → Microphone, then try again.", "麦克风权限已关闭。请在“系统设置 → 隐私与安全性 → 麦克风”中允许 Cortex，然后重试。"],
      ["Open microphone settings", "打开麦克风设置"],
      ["Try again", "重试"],
      ["Can't use the mic?", "无法使用麦克风？"],
      ["Upload an audio file", "上传音频文件"],
    ];

    expectTranslations(cases);
  });

  test("translates cheatsheet empty and failure states in Chinese mode", () => {
    const cases: Array<[string, string]> = [
      ["Whole subject", "整门课程"],
      ["Couldn't generate", "无法生成"],
      ["Couldn't generate cheatsheet", "无法生成速查表"],
      ["Generating cheatsheet…", "正在生成速查表…"],
      ["cheatsheet", "速查表"],
      ["model returned unstructured output; try again", "模型返回的内容格式不正确，请重试"],
      ["A completeness-checked cheatsheet will be generated from", "将根据"],
      ["this subject's", "本课程的"],
      ["this topic's", "当前主题的"],
      [
        "A completeness-checked cheatsheet will be generated from this subject's 5 sources.",
        "将根据本课程的 5 个来源生成经过完整性检查的速查表。",
      ],
      ["Regenerate all topic cheatsheets?", "重新生成所有主题的速查表？"],
      [
        "This generates a fresh cheatsheet for each of the 1 topic with sources (including the ungrouped “General” sources), running in parallel. It overwrites the existing cheatsheets and uses AI tokens for every source.",
        "这会为 1 个有来源的主题分别生成新的速查表（包括未分组的“常规”来源），并行运行。现有速查表将被覆盖，并会为每个来源消耗 AI Token。",
      ],
      ["Regenerate all", "全部重新生成"],
      ["Generate cheatsheet", "生成速查表"],
      ["Synthesizing…", "正在生成…"],
    ];

    expectTranslations(cases);
  });

  test("translates analytics, calendar, source ingestion and leader-menu copy", () => {
    const cases: Array<[string, string]> = [
      ["Accuracy · 7d", "正确率 · 7天"],
      ["Consistency · 30d", "坚持度 · 30天"],
      ["Focus minutes · last 90d", "专注分钟 · 最近 90 天"],
      ["2/7 days", "2/7 天"],
      ["48m", "48分钟"],
      ["August 2026", "2026年8月"],
      ["Wednesday, August 26", "8月26日 周三"],
      ["Aug 24 – 30, 2026", "2026年8月24日至30日"],
      ["Jul 27 – Aug 2, 2026", "2026年7月27日至8月2日"],
      ["Upload Files", "上传文件"],
      ["Paste URL", "粘贴网址"],
      ["web page · YouTube", "网页 · YouTube"],
      ["Paste Text", "粘贴文本"],
      ["markdown · plain text", "Markdown · 纯文本"],
      ["Record Lecture", "录制课程"],
      ["live audio + transcript", "实时音频 + 转写"],
      ["Snap Photo", "拍摄照片"],
      ["OCR a whiteboard / page", "识别白板 / 页面文字"],
      ["Space leader — context actions", "空格引导菜单 — 当前页面操作"],
      ["Record", "录音"],
      ["Review cheatsheet", "查看速查表"],
      ["Music", "音乐"],
      ["Pomodoro", "番茄钟"],
      ["Sidebar", "侧边栏"],
      ["view all sources", "查看全部来源"],
      ["open chat dock", "打开对话面板"],
      ["study analytics", "学习分析"],
      ["open preferences", "打开设置"],
      ["Ask", "提问"],
      ["actions", "操作"],
      ["help", "帮助"],
      ["command", "命令"],
      ["lo-fi · ad-free study", "低保真音乐 · 无广告学习"],
    ];

    expectTranslations(cases);
  });

  test("keeps critical setup and recording views free of untranslated static copy", () => {
    expect(findUntranslatedStaticCopy([
      "src/views/AddSubject.svelte",
      "src/views/Settings.svelte",
      "src/views/Recorder.svelte",
      "src/views/Cheatsheet.svelte",
      "src/components/GeneratingCard.svelte",
      "src/views/SubjectView.svelte",
      "src/components/Dialog.svelte",
      "src/views/AnalyticsView.svelte",
      "src/views/CalendarView.svelte",
      "src/views/AddSource.svelte",
      "src/components/LeaderPane.svelte",
      "src/App.svelte",
      "src/components/StatusBar.svelte",
      "src/components/Sidebar.svelte",
    ])).toEqual([]);
  });

  test("keeps every Svelte UI file free of untranslated static copy", () => {
    const files = svelteFilesUnder("src");
    const untranslated = findUntranslatedStaticCopy(files);
    expect(files).toHaveLength(58);
    expect(untranslated).toEqual([]);
  });
});
