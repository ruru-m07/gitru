use git2::StatusOptions;

pub fn default_status_options() -> StatusOptions {
    let mut opts = StatusOptions::new();
    opts.include_ignored(true)
        .include_unmodified(true)
        .include_unreadable(true)
        .include_unreadable_as_untracked(true)
        .include_untracked(true)
        .renames_index_to_workdir(true)
        .renames_head_to_index(true)
        .recurse_untracked_dirs(true);
    opts
}
