// cargo-generate pre-hook (also what `cargo leptos new` runs): ask for
// what the rename needs, then hand it to scripts/rename-app.sh, which
// works without a git repository — a generated tree has none yet.
// Running it needs --allow-commands, or a yes at the confirmation prompt.
//
// Prompting belongs here, not in the script: system::command captures the
// child's output, so a prompt inside bash would never reach the terminal.
// --define on the command line skips the matching question.
let slug = variable::get("project-name");

if !variable::is_set("app_name") {
    variable::set("app_name", variable::prompt(
        "Display name (window title, bundle productName)", slug.to_title_case()));
}
if !variable::is_set("identifier") {
    variable::set("identifier", variable::prompt(
        "Bundle identifier (reverse DNS)", `com.mycompany.${slug}`,
        "^[a-z][a-z0-9]*(\\.[a-z0-9-]+)+$"));
}
if !variable::is_set("author") {
    // Empty: the script falls back to git config user.name/user.email.
    variable::set("author", variable::prompt(
        "Author (name and email, empty = your git config)", ""));
}

// The values travel through a file, not the argument list: system::command
// goes through a shell, which would split "Ada L <ada@acme.com>" apart.
let args_file = ".rename-args";
file::write(args_file, [
    `name=${variable::get("app_name")}`,
    `slug=${slug}`,
    `repo=${slug}`,
    `identifier=${variable::get("identifier")}`,
    `author=${variable::get("author")}`,
]);

// --remove-self drops the rename script, this hook and cargo-generate.toml.
system::command("bash", ["scripts/rename-app.sh", "--args-file", args_file, "--remove-self"]);

file::delete(args_file);
