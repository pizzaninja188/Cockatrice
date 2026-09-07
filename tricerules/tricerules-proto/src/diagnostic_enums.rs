//! Enum labels for diagnostic serializers, generated from the very same protocol descriptor.

use prost::Message;
use prost_types::{DescriptorProto, EnumDescriptorProto, FileDescriptorSet};
use std::collections::BTreeMap;
use std::sync::OnceLock;

type Labels = BTreeMap<i32, String>;
type FieldLabels = BTreeMap<(String, String), Labels>;

fn collect_enums(
    prefix: &str,
    enums: &[EnumDescriptorProto],
    labels: &mut BTreeMap<String, Labels>,
) {
    for item in enums {
        labels.insert(
            format!("{prefix}.{}", item.name()),
            item.value
                .iter()
                .map(|v| (v.number(), v.name().to_owned()))
                .collect(),
        );
    }
}

fn collect_nested(
    prefix: &str,
    messages: &[DescriptorProto],
    labels: &mut BTreeMap<String, Labels>,
) {
    for message in messages {
        let name = format!("{prefix}.{}", message.name());
        collect_enums(&name, &message.enum_type, labels);
        collect_nested(&name, &message.nested_type, labels);
    }
}

fn collect_fields(
    messages: &[DescriptorProto],
    labels: &BTreeMap<String, Labels>,
    fields: &mut FieldLabels,
) {
    for message in messages {
        for field in &message.field {
            if let Some(values) = labels.get(field.type_name()) {
                fields.insert(
                    (message.name().to_owned(), field.name().to_owned()),
                    values.clone(),
                );
            }
        }
        collect_fields(&message.nested_type, labels, fields);
    }
}

pub fn name(message: &str, field: &str, number: i32) -> Option<&'static str> {
    static FIELDS: OnceLock<FieldLabels> = OnceLock::new();
    let fields = FIELDS.get_or_init(|| {
        let descriptors = FileDescriptorSet::decode(
            include_bytes!(concat!(env!("OUT_DIR"), "/ruled-descriptors.bin")).as_slice(),
        )
        .expect("build-generated descriptor");
        let mut labels = BTreeMap::new();
        for file in &descriptors.file {
            let prefix = format!(".{}", file.package());
            collect_enums(&prefix, &file.enum_type, &mut labels);
            collect_nested(&prefix, &file.message_type, &mut labels);
        }
        let mut fields = BTreeMap::new();
        for file in &descriptors.file {
            collect_fields(&file.message_type, &labels, &mut fields);
        }
        fields
    });
    fields
        .get(&(message.to_owned(), field.to_owned()))?
        .get(&number)
        .map(String::as_str)
}
