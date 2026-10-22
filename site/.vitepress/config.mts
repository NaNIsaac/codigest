import { defineConfig, type DefaultTheme } from 'vitepress'

const repo = 'https://github.com/NaNIsaac/codigest'

type Labels = {
  start: string
  gettingStarted: string
  installation: string
  reference: string
  commands: string
  configuration: string
  outputFormats: string
  terminalCapture: string
  mcp: string
  guides: string
  recipes: string
  sharing: string
  changes: string
  debugging: string
  security: string
  project: string
  architecture: string
  rustPort: string
  contributing: string
}

// Guides are sections of the recipes page; the Korean headings carry the same ASCII ids.
function sidebar(prefix: string, t: Labels): DefaultTheme.Sidebar {
  return [
    {
      text: t.start,
      items: [
        { text: t.installation, link: `${prefix}/installation` },
        { text: t.gettingStarted, link: `${prefix}/getting-started` },
      ],
    },
    {
      text: t.guides,
      items: [
        {
          text: t.recipes,
          link: `${prefix}/recipes`,
          items: [
            { text: t.sharing, link: `${prefix}/recipes#share-one-folder-or-one-question-s-worth-of-files` },
            { text: t.changes, link: `${prefix}/recipes#review-a-refactor-with-mark-and-semdiff` },
            { text: t.debugging, link: `${prefix}/recipes#a-test-fails-and-you-do-not-know-why` },
          ],
        },
        { text: t.mcp, link: `${prefix}/mcp` },
      ],
    },
    {
      text: t.reference,
      items: [
        { text: t.commands, link: `${prefix}/commands` },
        { text: t.configuration, link: `${prefix}/configuration` },
        { text: t.outputFormats, link: `${prefix}/output-formats` },
        { text: t.terminalCapture, link: `${prefix}/terminal-capture` },
      ],
    },
    {
      text: t.project,
      items: [
        { text: t.security, link: `${prefix}/security` },
        { text: t.architecture, link: `${prefix}/architecture` },
        { text: t.rustPort, link: `${prefix}/rust-port` },
        { text: t.contributing, link: `${prefix}/contributing` },
      ],
    },
  ]
}

const en: Labels = {
  start: 'Getting Started',
  gettingStarted: 'Quick Start',
  installation: 'Installation',
  reference: 'Reference',
  commands: 'Command Reference',
  configuration: 'Configuration',
  outputFormats: 'Output Formats',
  terminalCapture: 'Terminal Capture',
  mcp: 'MCP Integration',
  guides: 'Guides',
  recipes: 'Recipes',
  sharing: 'Sharing code with an AI',
  changes: 'Working with changes',
  debugging: 'Debugging a failed command',
  security: 'Security',
  project: 'Project',
  architecture: 'Architecture',
  rustPort: 'Rust and Python Builds',
  contributing: 'Contributing',
}

const ko: Labels = {
  start: '시작하기',
  gettingStarted: '빠른 시작',
  installation: '설치',
  reference: '레퍼런스',
  commands: '명령어 레퍼런스',
  configuration: '설정',
  outputFormats: '출력 형식',
  terminalCapture: '터미널 기록',
  mcp: 'MCP 연동',
  guides: '가이드',
  recipes: '활용 예시',
  sharing: 'AI에게 코드 보여 주기',
  changes: '바뀐 부분 다루기',
  debugging: '실패한 명령 디버깅',
  security: '보안',
  project: '프로젝트',
  architecture: '구조',
  rustPort: 'Rust와 Python 빌드',
  contributing: '기여하기',
}

export default defineConfig({
  title: 'Codigest',
  base: '/codigest/',
  cleanUrls: true,
  lastUpdated: true,
  head: [['link', { rel: 'icon', type: 'image/svg+xml', href: '/codigest/favicon.svg' }]],

  locales: {
    root: {
      label: 'English',
      lang: 'en',
      description:
        'Share the code context you choose with an AI: your project, a few files, what changed, or the command that failed.',
      themeConfig: {
        nav: [
          { text: 'Getting Started', link: '/installation', activeMatch: '^/(getting-started|installation)' },
          { text: 'Guides', link: '/recipes', activeMatch: '^/(recipes|mcp)' },
          {
            text: 'Reference',
            link: '/commands',
            activeMatch: '^/(commands|configuration|output-formats|terminal-capture)',
          },
          { text: 'Security', link: '/security' },
          {
            text: 'More',
            items: [
              { text: 'Architecture', link: '/architecture' },
              { text: 'Rust and Python Builds', link: '/rust-port' },
              { text: 'Contributing', link: '/contributing' },
            ],
          },
        ],
        sidebar: sidebar('', en),
        editLink: {
          pattern: `${repo}/edit/main/site/:path`,
          text: 'Edit this page on GitHub',
        },
        lastUpdated: { text: 'Last updated' },
      },
    },
    ko: {
      label: '한국어',
      lang: 'ko',
      link: '/ko/',
      description:
        'AI에게 보여 줄 코드를 내가 고르게 해 주는 도구. 프로젝트 전체든, 파일 몇 개든, 바뀐 부분이든, 실패한 명령이든.',
      themeConfig: {
        nav: [
          { text: '시작하기', link: '/ko/installation', activeMatch: '^/ko/(getting-started|installation)' },
          { text: '가이드', link: '/ko/recipes', activeMatch: '^/ko/(recipes|mcp)' },
          {
            text: '레퍼런스',
            link: '/ko/commands',
            activeMatch: '^/ko/(commands|configuration|output-formats|terminal-capture)',
          },
          { text: '보안', link: '/ko/security' },
          {
            text: '더 보기',
            items: [
              { text: '구조', link: '/ko/architecture' },
              { text: 'Rust와 Python 빌드', link: '/ko/rust-port' },
              { text: '기여하기', link: '/ko/contributing' },
            ],
          },
        ],
        sidebar: sidebar('/ko', ko),
        editLink: {
          pattern: `${repo}/edit/main/site/:path`,
          text: 'GitHub에서 이 페이지 수정하기',
        },
        lastUpdated: { text: '마지막 수정' },
        docFooter: { prev: '이전', next: '다음' },
        outline: { label: '이 페이지에서', level: [2, 3] },
        returnToTopLabel: '맨 위로',
        sidebarMenuLabel: '메뉴',
        darkModeSwitchLabel: '화면 모드',
        lightModeSwitchTitle: '라이트 모드로 전환',
        darkModeSwitchTitle: '다크 모드로 전환',
        langMenuLabel: '언어 변경',
        skipToContentLabel: '본문으로 건너뛰기',
      },
    },
  },

  themeConfig: {
    logo: { src: '/favicon.svg', alt: 'Codigest' },
    siteTitle: 'Codigest',
    socialLinks: [{ icon: 'github', link: repo }],
    outline: { level: [2, 3] },
    search: {
      provider: 'local',
      options: {
        locales: {
          ko: {
            translations: {
              button: { buttonText: '검색', buttonAriaLabel: '검색' },
              modal: {
                displayDetails: '자세히 보기',
                resetButtonTitle: '검색어 지우기',
                backButtonTitle: '뒤로',
                noResultsText: '결과가 없습니다',
                footer: {
                  selectText: '선택',
                  selectKeyAriaLabel: '엔터',
                  navigateText: '이동',
                  navigateUpKeyAriaLabel: '위쪽 화살표',
                  navigateDownKeyAriaLabel: '아래쪽 화살표',
                  closeText: '닫기',
                  closeKeyAriaLabel: 'esc',
                },
              },
            },
          },
        },
      },
    },
    footer: {
      message: 'Released under the MIT License.',
      copyright: 'Codigest',
    },
  },
})
