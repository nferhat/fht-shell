fn main() {
    glib_build_tools::compile_resources(
        &["resources"],
        "resources/resources.gresource.xml",
        "fht.desktop.Shell.gresource",
    );

    glib_build_tools::compile_resources(
        &["resources/icons"],
        "resources/icons/resources.gresource.xml",
        "fht.desktop.Shell.icons.gresource",
    );
}
