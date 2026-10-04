//! Print the generated ProseMirror schema; redirect into `schema/prosemirror-schema.json`.

fn main() {
    println!("{}", cdoc_pm::schema::schema_json());
}
