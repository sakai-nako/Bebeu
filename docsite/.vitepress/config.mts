import { defineConfig } from 'vitepress'

// VitePress 設定。ローカル閲覧用 (公開デプロイなし) のユーザーマニュアル。
export default defineConfig({
  lang: 'ja-JP',
  title: 'Bebeu docs',
  description: 'editor / engine ユーザーマニュアル',
  lastUpdated: true,
  cleanUrls: true,

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
        },
      },
    },

    outline: { label: '目次', level: [2, 3] },
    docFooter: { prev: '前へ', next: '次へ' },
    lastUpdatedText: '最終更新',
    darkModeSwitchLabel: 'テーマ切替',
    sidebarMenuLabel: 'メニュー',
    returnToTopLabel: 'トップへ戻る',
  },
})
