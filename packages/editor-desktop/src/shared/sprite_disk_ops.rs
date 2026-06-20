/// SpriteGroupEditor の「未コミットの画像 disk 操作」を表す保留状態。
///
/// Editor セッション内で起きた画像のコピー / 削除予定をここに集約しておき、
/// Save 時にコミット（commit）、Cancel / unmount 時にロールバック（rollback）する。
/// これによって import / 差し替えが atomic（Cancel すると disk が元の状態に戻る）になる。
///
/// - `pending_imports`: disk 上に存在するが yml には登録されていない画像
/// - `pending_deletions`: yml にはまだ登録されているが、commit 時に削除予定の画像
/// - `pending_overwrites`: 上書き import で `{basename}.bak` のバックアップが取られている画像。
///   commit で .bak を削除、rollback で .bak から復元する
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[allow(clippy::struct_field_names)]
pub struct SpriteDiskOps {
    /// このセッション内で disk にコピーされたが、まだ yml に commit されていない画像 basenames。
    /// - Save 成功 → このリストはクリア（commit 完了）
    /// - Cancel / unmount → 全エントリを `delete_sprite_image` で除去（rollback）
    pub pending_imports: Vec<String>,
    /// このセッション内で「削除予定」とマークされた既存画像 basenames。
    /// - Save 成功 → 全エントリを `delete_sprite_image` で実際に消す
    /// - Cancel / unmount → 何もしない（disk に残す = 元の状態のまま）
    pub pending_deletions: Vec<String>,
    /// このセッション内で同名上書き import が行われ、`{basename}.bak` に旧ファイルが退避されている画像 basenames。
    /// - Save 成功 → 全エントリを `discard_sprite_image_backup` で .bak を消す（上書き確定）
    /// - Cancel / unmount → 全エントリを `restore_sprite_image_backup` で .bak から戻す（rollback）
    pub pending_overwrites: Vec<String>,
}

impl SpriteDiskOps {
    /// `pending_imports` に basename を追加する。
    pub fn add_pending_import(&mut self, basename: String) {
        self.pending_imports.push(basename);
    }

    /// `pending_imports` から basename を見つけて取り除く。同セッション内で取り消す
    /// 操作（差し替えで旧画像が今回 import 分だった場合の即削除）に使う。
    /// 見つかれば true、無ければ false。
    pub fn take_pending_import(&mut self, basename: &str) -> bool {
        if let Some(pos) = self.pending_imports.iter().position(|p| p == basename) {
            self.pending_imports.remove(pos);
            true
        } else {
            false
        }
    }

    /// `pending_deletions` に basename を追加する。Save 時に削除される。
    pub fn add_pending_deletion(&mut self, basename: String) {
        self.pending_deletions.push(basename);
    }

    /// `pending_overwrites` に basename を追加する。
    /// 既に同じ basename が登録済みなら追加しない（同セッションで同 basename を 2 回上書き
    /// しても、対応する .bak は最初の取り込み前のものだけが保持されているため、エントリは 1 つで十分）。
    pub fn add_pending_overwrite(&mut self, basename: String) {
        if !self.pending_overwrites.iter().any(|b| b == &basename) {
            self.pending_overwrites.push(basename);
        }
    }

    /// 保留中の操作が無ければ true。Save 後の commit クリアで使う。
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.pending_imports.is_empty()
            && self.pending_deletions.is_empty()
            && self.pending_overwrites.is_empty()
    }
}
