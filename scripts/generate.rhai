// cargo-generate pre-hook: hand the prompted values to
// scripts/rename-app.sh, which does the whole rename (it works without a
// git repository, which is what a generated tree is at this point).
// Requires --allow-commands, or approve the command when prompted.
//
// The values travel through a file, not the argument list: system::command
// goes through a shell, which would split "Ada L <ada@acme.com>" apart.
let args_file = ".rename-args";
file::write(args_file, [
    `name=${variable::get("app_name")}`,
    `slug=${variable::get("project-name")}`,
    `repo=${variable::get("project-name")}`,
    `identifier=${variable::get("identifier")}`,
    `author=${variable::get("author")}`,
]);

// --remove-self drops the rename script, this hook and cargo-generate.toml.
system::command("bash", ["scripts/rename-app.sh", "--args-file", args_file, "--remove-self"]);

file::delete(args_file);
