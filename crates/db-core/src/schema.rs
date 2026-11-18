use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ObjectKind {
    #[default]
    Table,
    View,
    Function,
    Procedure,
    Trigger,
}

impl ObjectKind {
    pub fn singular(self) -> &'static str {
        match self {
            ObjectKind::Table => "Tabella",
            ObjectKind::View => "Vista",
            ObjectKind::Function => "Funzione",
            ObjectKind::Procedure => "Procedura",
            ObjectKind::Trigger => "Trigger",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            ObjectKind::Table => "Tabelle",
            ObjectKind::View => "Viste",
            ObjectKind::Function => "Funzioni",
            ObjectKind::Procedure => "Procedure",
            ObjectKind::Trigger => "Trigger",
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct SchemaObject {
    pub name: String,
    pub kind: ObjectKind,
    pub rows: Option<i64>,
    pub comment: Option<String>,
    /// Next AUTO_INCREMENT / identity value (tables).
    pub auto_increment: Option<i64>,
    /// Last change of the data or the structure, as the server reports it.
    pub modified: Option<String>,
    /// Size of the data on disk, in bytes.
    pub data_length: Option<i64>,
    /// Storage engine (MySQL/MariaDB).
    pub engine: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct ColumnInfo {
    pub name: String,
    pub data_type: String,
    pub nullable: bool,
    pub default: Option<String>,
    pub primary_key: bool,
    pub auto_increment: bool,
    pub comment: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct IndexInfo {
    pub name: String,
    pub columns: Vec<String>,
    pub unique: bool,
}

#[derive(Clone, Debug, Default)]
pub struct ForeignKeyInfo {
    pub name: String,
    pub columns: Vec<String>,
    pub ref_table: String,
    pub ref_columns: Vec<String>,
    pub on_delete: Option<String>,
    pub on_update: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct TableInfo {
    pub schema: Option<String>,
    pub name: String,
    pub columns: Vec<ColumnInfo>,
    pub indexes: Vec<IndexInfo>,
    pub foreign_keys: Vec<ForeignKeyInfo>,
}

impl TableInfo {
    pub fn primary_key(&self) -> Vec<String> {
        self.columns.iter().filter(|c| c.primary_key).map(|c| c.name.clone()).collect()
    }
}

/// Editable column in the table designer.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ColumnDef {
    pub name: String,
    pub data_type: String,
    pub not_null: bool,
    pub default: String,
    pub primary_key: bool,
    pub auto_increment: bool,
    pub comment: String,
    /// Name the column had when the design was loaded (`None`: new column).
    pub original: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct IndexDef {
    pub name: String,
    pub columns: String,
    pub unique: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ForeignKeyDef {
    pub name: String,
    pub columns: String,
    pub ref_table: String,
    pub ref_columns: String,
    pub on_delete: String,
    pub on_update: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TableDesign {
    pub schema: Option<String>,
    pub name: String,
    pub columns: Vec<ColumnDef>,
    pub indexes: Vec<IndexDef>,
    pub foreign_keys: Vec<ForeignKeyDef>,
}

impl TableDesign {
    pub fn from_info(t: &TableInfo) -> Self {
        TableDesign {
            schema: t.schema.clone(),
            name: t.name.clone(),
            columns: t
                .columns
                .iter()
                .map(|c| ColumnDef {
                    name: c.name.clone(),
                    data_type: c.data_type.clone(),
                    not_null: !c.nullable,
                    default: c.default.clone().unwrap_or_default(),
                    primary_key: c.primary_key,
                    auto_increment: c.auto_increment,
                    comment: c.comment.clone().unwrap_or_default(),
                    original: Some(c.name.clone()),
                })
                .collect(),
            indexes: t
                .indexes
                .iter()
                .filter(|i| !i.name.eq_ignore_ascii_case("PRIMARY"))
                .map(|i| IndexDef { name: i.name.clone(), columns: i.columns.join(", "), unique: i.unique })
                .collect(),
            foreign_keys: t
                .foreign_keys
                .iter()
                .map(|f| ForeignKeyDef {
                    name: f.name.clone(),
                    columns: f.columns.join(", "),
                    ref_table: f.ref_table.clone(),
                    ref_columns: f.ref_columns.join(", "),
                    on_delete: f.on_delete.clone().unwrap_or_default(),
                    on_update: f.on_update.clone().unwrap_or_default(),
                })
                .collect(),
        }
    }
}
