import { defineConfig } from 'vitepress'

// VitePress 設定。ローカル閲覧用 (公開デプロイなし) のユーザーマニュアル。
// i18n 構成: root を 日本語 (ja-JP)、`/en/` を英語 (en-US) として 2 言語対応。
// 言語切り替えは VitePress 標準の右上セレクタ + 同名パス自動リンクで動く。
//
// 共通設定 (search 等) は top-level themeConfig に置き、locale 個別の UI labels と
// sidebar/nav は locales[].themeConfig で上書きする。
export default defineConfig({
  title: 'Bebeu docs',
  description: 'editor / engine ユーザーマニュアル',
  lastUpdated: true,
  cleanUrls: true,

  themeConfig: {
    search: {
      provider: 'local',
      options: {
        locales: {
          root: {
            translations: {
              button: { buttonText: '検索', buttonAriaLabel: '検索' },
              modal: {
                noResultsText: '一致する結果がありません',
                resetButtonTitle: '検索をリセット',
                footer: {
                  selectText: '選択',
                  navigateText: '移動',
                  closeText: '閉じる',
                },
              },
            },
          },
          en: {
            translations: {
              button: { buttonText: 'Search', buttonAriaLabel: 'Search' },
              modal: {
                noResultsText: 'No results for',
                resetButtonTitle: 'Reset search',
                footer: {
                  selectText: 'to select',
                  navigateText: 'to navigate',
                  closeText: 'to close',
                },
              },
            },
          },
        },
      },
    },
  },

  locales: {
    root: {
      label: '日本語',
      lang: 'ja-JP',
      themeConfig: {
        nav: [
          { text: 'Home', link: '/' },
          { text: 'editor', link: '/editor/' },
          { text: 'engine', link: '/engine/' },
        ],

        sidebar: {
          '/editor/': [
            {
              text: 'editor',
              items: [
                { text: '概要', link: '/editor/' },
                { text: 'セットアップと起動', link: '/editor/setup' },
                { text: 'Workspace の選択', link: '/editor/workspace' },
                { text: 'Project', link: '/editor/project' },
                { text: 'Level', link: '/editor/level' },
                { text: 'キーバインド', link: '/editor/keybindings' },
                { text: 'ユーザー設定', link: '/editor/preferences' },
              ],
            },
          ],
          '/engine/': [
            {
              text: 'engine',
              items: [
                { text: '概要', link: '/engine/' },
                { text: '起動と Project 指定', link: '/engine/run' },
                { text: '操作方法', link: '/engine/controls' },
                { text: 'Debug ビルド', link: '/engine/debug' },
              ],
            },
          ],
        },

        outline: { label: '目次', level: [2, 3] },
        docFooter: { prev: '前へ', next: '次へ' },
        lastUpdatedText: '最終更新',
        darkModeSwitchLabel: 'テーマ切替',
        sidebarMenuLabel: 'メニュー',
        returnToTopLabel: 'トップへ戻る',
        langMenuLabel: '言語を切り替え',
      },
    },

    en: {
      label: 'English',
      lang: 'en-US',
      link: '/en/',
      themeConfig: {
        nav: [
          { text: 'Home', link: '/en/' },
          { text: 'editor', link: '/en/editor/' },
          { text: 'engine', link: '/en/engine/' },
        ],

        sidebar: {
          '/en/editor/': [
            {
              text: 'editor',
              items: [
                { text: 'Overview', link: '/en/editor/' },
                { text: 'Setup and launch', link: '/en/editor/setup' },
                { text: 'Selecting a workspace', link: '/en/editor/workspace' },
                { text: 'Project', link: '/en/editor/project' },
                { text: 'Level', link: '/en/editor/level' },
                { text: 'Key bindings', link: '/en/editor/keybindings' },
                { text: 'User preferences', link: '/en/editor/preferences' },
              ],
            },
          ],
          '/en/engine/': [
            {
              text: 'engine',
              items: [
                { text: 'Overview', link: '/en/engine/' },
                { text: 'Launching and project selection', link: '/en/engine/run' },
                { text: 'Controls', link: '/en/engine/controls' },
                { text: 'Debug build', link: '/en/engine/debug' },
              ],
            },
          ],
        },

        outline: { label: 'On this page', level: [2, 3] },
        docFooter: { prev: 'Previous', next: 'Next' },
        lastUpdatedText: 'Last updated',
        darkModeSwitchLabel: 'Theme',
        sidebarMenuLabel: 'Menu',
        returnToTopLabel: 'Back to top',
        langMenuLabel: 'Change language',
      },
    },
  },
})
