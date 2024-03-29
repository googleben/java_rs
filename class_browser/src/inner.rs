use self::glib::StaticType;
use self::glib::ToValue;
use gtk::*;
use gtk::prelude::CellLayoutExt;
use gtk::prelude::TreeStoreExtManual;
use gtk::prelude::TreeViewExt;
use java_class::attributes;
use java_class::attributes::Annotation;
use java_class::attributes::Attribute::*;
use java_class::attributes::Attribute;
use java_class::attributes::ElementValue;
use java_class::attributes::StackMapFrame;
use java_class::attributes::TargetInfo;
use java_class::attributes::TypeAnnotation;
use java_class::attributes::VerificationTypeInfo;
use java_class::class::AccessFlags;
use java_class::class::JavaClass;
use java_class::cp::CPIndex;
use java_class::cp::ConstantPool;
use java_class::cp_info::CPInfo;
use java_class::fields;
use java_class::methods;
use java_class::opcodes::Opcode;
use java_class::opcodes::Opcode::*;
use std::str;

pub fn insert_with_values(ts: &TreeStore, parent: Option<&TreeIter>,
    position: Option<u32>,
    columns: &[u32], values: &[&dyn ToValue]) -> TreeIter {
        let mut ans = Vec::with_capacity(columns.len());
        for i in 0..columns.len() {
            ans.push((columns[i], values[i]));
        }
        ts.insert_with_values(parent, position, &ans)
    }

pub fn class_to_tree(class: JavaClass) -> TreeView {
    let tree = TreeView::new();

    let column = TreeViewColumn::new();
    let cell = CellRendererText::new();

    column.pack_start(&cell, true);
    column.add_attribute(&cell, "text", 0);
    tree.append_column(&column);

    let col2 = TreeViewColumn::new();
    col2.pack_start(&cell, true);
    col2.add_attribute(&cell, "text", 1);
    tree.append_column(&col2);

    tree.set_headers_visible(false);

    let ans = TreeStore::new(&[String::static_type(), String::static_type()]);
    let name = get_name(&class.constant_pool, class.this_class);
    let iter = insert_with_values(&ans, None, None, &[0, 1], &[&"Class", &name]);
    insert_with_values(&ans, Some(&iter), None, &[0, 1], &[&"minor_version", &format!("{}", class.minor_version)]);
    insert_with_values(&ans, Some(&iter), None, &[0, 1], &[&"major_version", &format!("{}", class.major_version)]);
    let constants = &class.constant_pool;
    insert_constant_pool(&ans, &iter, constants);
    insert_access_class(&ans, &iter, class.access_flags);
    insert_with_values(&ans, Some(&iter), None, &[0, 1], &[&"this_class", &format!("{}", class.this_class)]);
    insert_with_values(&ans, Some(&iter), None, &[0, 1], &[&"super_class", &(format!("{} ({})", class.super_class, if class.super_class.as_u16() == 0 { "".to_owned() } else { get_name(constants, class.super_class) }))]);
    let interfaces = insert_with_values(&ans, Some(&iter), None, &[0, 1], &[&"interfaces", &""]);
    for interface in class.interfaces {
        let name = get_name(&class.constant_pool, interface);
        insert_with_values(&ans, Some(&interfaces), None, &[0, 1], &[&format!("{}", interface), &name.to_string()]);
    }
    let fields = insert_with_values(&ans, Some(&iter), None, &[0, 1], &[&"Fields", &""]);
    for f in class.fields {
        let field = insert_with_values(&ans, Some(&fields), None, &[0, 1], &[&"Field", &format!("{} {}", get_name(&class.constant_pool, f.name_index), get_name(&class.constant_pool, f.descriptor_index))]);
        insert_access_field(&ans, &field, f.access_flags);
        insert_with_values(&ans, Some(&field), None, &[0, 1], &[&"name_index", &format!("{}", f.name_index)]);
        insert_with_values(&ans, Some(&field), None, &[0, 1], &[&"descriptor_index", &format!("{}", f.descriptor_index)]);
        insert_attributes(constants, &ans, &field, f.attributes);
    }
    let methods = insert_with_values(&ans, Some(&iter), None, &[0, 1], &[&"Methods", &""]);
    for m in class.methods {
        let method = insert_with_values(&ans, Some(&methods), None, &[0, 1], &[&"Method", &format!("{}{}", get_name(&class.constant_pool, m.name_index), get_name(&class.constant_pool, m.descriptor_index))]);
        insert_access_method(&ans, &method, m.access_flags);
        insert_with_values(&ans, Some(&method), None, &[0, 1], &[&"name_index", &format!("{}", m.name_index)]);
        insert_with_values(&ans, Some(&method), None, &[0, 1], &[&"descriptor_index", &format!("{}", m.descriptor_index)]);
        insert_attributes(constants, &ans, &method, m.attributes);
    }
    insert_attributes(constants, &ans, &iter, class.attributes);
    tree.set_model(Some(&ans));
    tree
}

fn get_name(cp: &ConstantPool, index: CPIndex) -> String {
    match &cp[index] {
        CPInfo::Class { name_index } => {
            get_name(cp, *name_index)
        }
        CPInfo::Utf8 { bytes, .. } => { str::from_utf8(bytes.as_slice()).unwrap().to_owned() }
        CPInfo::String { string_index } => { get_name(cp, *string_index) }
        CPInfo::NameAndType { name_index, descriptor_index } => {
            get_name(cp, *name_index) + " " + &get_name(cp, *descriptor_index)
        }
        CPInfo::Methodref { class_index, name_and_type_index } |
        CPInfo::Fieldref { class_index, name_and_type_index } |
        CPInfo::InterfaceMethodref { class_index, name_and_type_index } => {
            get_name(cp, *class_index) + " " + &get_name(cp, *name_and_type_index)
        }
        CPInfo::Integer { bytes } => {
            format!("{}", *bytes as i32)
        }
        CPInfo::Float { bytes } => {
            format!("{}", f32::from_bits(*bytes))
        }
        CPInfo::Long { bytes } => {
            format!("{}", *bytes as i64)
        }
        CPInfo::Double { bytes } => {
            format!("{}", f64::from_bits(*bytes))
        }
        CPInfo::InvokeDynamic { name_and_type_index, .. } => {
            get_name(cp, *name_and_type_index)
        }
        _ => "Constant Pool index did not point to Utf8".to_owned()
    }
}

fn insert_constant_pool(store: &TreeStore, iter: &TreeIter, constants: &ConstantPool) {
    let cp = insert_with_values(store, Some(&iter), None, &[0, 1], &[&"Constant Pool", &""]);
    for i in 1..=constants.items().len() as u16 {
        let i = i.into();
        let cp_item = &constants[i];
        match cp_item {
            CPInfo::Class { name_index } => {
                let iter_n = insert_with_values(store, Some(&cp), None, &[0, 1], &[&format!("{}. Class", i), &get_name(constants, *name_index)]);
                insert_with_values(store, Some(&iter_n), None, &[0, 1], &[&"name_index", &format!("{}", name_index)]);
            }
            CPInfo::Fieldref { class_index, name_and_type_index } => {
                let iter_n = insert_with_values(store, Some(&cp), None, &[0, 1], &[&format!("{}. Fieldref", i), &get_name(constants, i)]);
                insert_with_values(store, Some(&iter_n), None, &[0, 1], &[&"class_index", &format!("{}", class_index)]);
                insert_with_values(store, Some(&iter_n), None, &[0, 1], &[&"name_and_type_index", &format!("{}", name_and_type_index)]);
            }
            CPInfo::Methodref { class_index, name_and_type_index } => {
                let iter_n = insert_with_values(store, Some(&cp), None, &[0, 1], &[&format!("{}. Methodref", i), &get_name(constants, i)]);
                insert_with_values(store, Some(&iter_n), None, &[0, 1], &[&"class_index", &format!("{}", class_index)]);
                insert_with_values(store, Some(&iter_n), None, &[0, 1], &[&"name_and_type_index", &format!("{}", name_and_type_index)]);
            }
            CPInfo::InterfaceMethodref { class_index, name_and_type_index } => {
                let iter_n = insert_with_values(store, Some(&cp), None, &[0, 1], &[&format!("{}. InterfaceMethodref", i), &get_name(constants, i)]);
                insert_with_values(store, Some(&iter_n), None, &[0, 1], &[&"class_index", &format!("{}", class_index)]);
                insert_with_values(store, Some(&iter_n), None, &[0, 1], &[&"name_and_type_index", &format!("{}", name_and_type_index)]);
            }
            CPInfo::String { string_index } => {
                let iter_n = insert_with_values(store, Some(&cp), None, &[0, 1], &[&format!("{}. String", i), &get_name(constants, *string_index)]);
                insert_with_values(store, Some(&iter_n), None, &[0, 1], &[&"string_index", &format!("{}", string_index)]);
            }
            CPInfo::Integer { bytes } => {
                let iter_n = insert_with_values(store, Some(&cp), None, &[0, 1], &[&format!("{}. Integer", i), &get_name(constants, i)]);
                insert_with_values(store, Some(&iter_n), None, &[0, 1], &[&"bytes", &format!("{}", bytes)]);
            }
            CPInfo::Float { bytes } => {
                let iter_n = insert_with_values(store, Some(&cp), None, &[0, 1], &[&format!("{}. Float", i), &get_name(constants, i)]);
                insert_with_values(store, Some(&iter_n), None, &[0, 1], &[&"bytes", &format!("{}", bytes)]);
            }
            CPInfo::Long { bytes } => {
                let iter_n = insert_with_values(store, Some(&cp), None, &[0, 1], &[&format!("{}. Long", i), &get_name(constants, i)]);
                insert_with_values(store, Some(&iter_n), None, &[0, 1], &[&"bytes", &format!("{}", bytes)]);
            }
            CPInfo::Double { bytes } => {
                let iter_n = insert_with_values(store, Some(&cp), None, &[0, 1], &[&format!("{}. Double", i), &get_name(constants, i)]);
                insert_with_values(store, Some(&iter_n), None, &[0, 1], &[&"bytes", &format!("{}", bytes)]);
            }
            CPInfo::NameAndType { name_index, descriptor_index } => {
                let iter_n = insert_with_values(store, Some(&cp), None, &[0, 1], &[&format!("{}. NameAndType", i), &(get_name(constants, i))]);
                insert_with_values(store, Some(&iter_n), None, &[0, 1], &[&"name_index", &format!("{}", name_index)]);
                insert_with_values(store, Some(&iter_n), None, &[0, 1], &[&"descriptor_index", &format!("{}", descriptor_index)]);
            }
            CPInfo::Utf8 { length, bytes } => {
                let iter_n = insert_with_values(store, Some(&cp), None, &[0, 1], &[&format!("{}. Utf8", i), &str::from_utf8(bytes).unwrap()]);
                insert_with_values(store, Some(&iter_n), None, &[0, 1], &[&"length", &format!("{}", length)]);
                let iter_bytes = insert_with_values(store, Some(&iter_n), None, &[0, 1], &[&"bytes", &str::from_utf8(bytes).unwrap()]);
                for byte in bytes {
                    insert_with_values(store, Some(&iter_bytes), None, &[0, 1], &[&format!("{}", byte), &""]);
                }
            }
            CPInfo::MethodHandle { reference_kind, reference_index } => {
                let iter_n = insert_with_values(store, Some(&cp), None, &[0, 1], &[&format!("{}. MethodHandle", i), &get_name(constants, *reference_index)]);
                insert_with_values(store, Some(&iter_n), None, &[0, 1], &[&"reference_kind", &format!("{}", reference_kind)]);
                insert_with_values(store, Some(&iter_n), None, &[0, 1], &[&"reference_index", &format!("{}", reference_index)]);
            }
            CPInfo::MethodType { descriptor_index } => {
                let iter_n = insert_with_values(store, Some(&cp), None, &[0, 1], &[&format!("{}. MethodType", i), &get_name(constants, *descriptor_index)]);
                insert_with_values(store, Some(&iter_n), None, &[0, 1], &[&"descriptor_index", &format!("{}", descriptor_index)]);
            }
            CPInfo::InvokeDynamic { bootstrap_method_attr_index, name_and_type_index } => {
                let iter_n = insert_with_values(store, Some(&cp), None, &[0, 1], &[&format!("{}. InvokeDynamic", i), &get_name(constants, *name_and_type_index)]);
                insert_with_values(store, Some(&iter_n), None, &[0, 1], &[&"bootstrap_method_attr_index", &format!("{}", bootstrap_method_attr_index)]);
                insert_with_values(store, Some(&iter_n), None, &[0, 1], &[&"name_and_type_index", &format!("{}", name_and_type_index)]);
            }
            CPInfo::LongDoubleDummy => {
                let iter_n = insert_with_values(store, Some(&cp), None, &[0, 1], &[&format!("{}. Long/Double Dummy Entry", i), &""]);
                insert_with_values(store, Some(&iter_n), None, &[0, 1], &[&"Due to extremely poor choices by the original JVM architects, Longs and Doubles are 2 constant pool entries", &""]);
            }
        }
    }
}

fn insert_access_class(store: &TreeStore, iter: &TreeIter, access_flags: u16) {
    let access = insert_with_values(store, Some(&iter), None, &[0, 1], &[&"access_flags", &format!("{:#06X}", access_flags)]);
    insert_with_values(store, Some(&access), None, &[0, 1], &[&"public", &format!("{}", (access_flags & AccessFlags::Public as u16) != 0)]);
    insert_with_values(store, Some(&access), None, &[0, 1], &[&"final", &format!("{}", (access_flags & AccessFlags::Final as u16) != 0)]);
    insert_with_values(store, Some(&access), None, &[0, 1], &[&"super", &format!("{}", (access_flags & AccessFlags::Super as u16) != 0)]);
    insert_with_values(store, Some(&access), None, &[0, 1], &[&"interface", &format!("{}", (access_flags & AccessFlags::Interface as u16) != 0)]);
    insert_with_values(store, Some(&access), None, &[0, 1], &[&"abstract", &format!("{}", (access_flags & AccessFlags::Abstract as u16) != 0)]);
    insert_with_values(store, Some(&access), None, &[0, 1], &[&"synthetic", &format!("{}", (access_flags & AccessFlags::Synthetic as u16) != 0)]);
    insert_with_values(store, Some(&access), None, &[0, 1], &[&"annotation", &format!("{}", (access_flags & AccessFlags::Annotation as u16) != 0)]);
    insert_with_values(store, Some(&access), None, &[0, 1], &[&"enum", &format!("{}", (access_flags & AccessFlags::Enum as u16) != 0)]);
}

fn insert_access_field(store: &TreeStore, iter: &TreeIter, access_flags: u16) {
    let access = insert_with_values(store, Some(&iter), None, &[0, 1], &[&"access_flags", &format!("{:#06X}", access_flags)]);
    insert_with_values(store, Some(&access), None, &[0, 1], &[&"public", &format!("{}", (access_flags & fields::AccessFlags::Public as u16) != 0)]);
    insert_with_values(store, Some(&access), None, &[0, 1], &[&"private", &format!("{}", (access_flags & fields::AccessFlags::Private as u16) != 0)]);
    insert_with_values(store, Some(&access), None, &[0, 1], &[&"protected", &format!("{}", (access_flags & fields::AccessFlags::Protected as u16) != 0)]);
    insert_with_values(store, Some(&access), None, &[0, 1], &[&"static", &format!("{}", (access_flags & fields::AccessFlags::Static as u16) != 0)]);
    insert_with_values(store, Some(&access), None, &[0, 1], &[&"final", &format!("{}", (access_flags & fields::AccessFlags::Final as u16) != 0)]);
    insert_with_values(store, Some(&access), None, &[0, 1], &[&"volatile", &format!("{}", (access_flags & fields::AccessFlags::Volatile as u16) != 0)]);
    insert_with_values(store, Some(&access), None, &[0, 1], &[&"transient", &format!("{}", (access_flags & fields::AccessFlags::Transient as u16) != 0)]);
    insert_with_values(store, Some(&access), None, &[0, 1], &[&"synthetic", &format!("{}", (access_flags & fields::AccessFlags::Synthetic as u16) != 0)]);
    insert_with_values(store, Some(&access), None, &[0, 1], &[&"enum", &format!("{}", (access_flags & fields::AccessFlags::Enum as u16) != 0)]);
}

fn insert_access_method(store: &TreeStore, iter: &TreeIter, access_flags: u16) {
    let access = insert_with_values(store, Some(&iter), None, &[0, 1], &[&"access_flags", &format!("{:#06X}", access_flags)]);
    insert_with_values(store, Some(&access), None, &[0, 1], &[&"public", &format!("{}", (access_flags & methods::AccessFlags::Public as u16) != 0)]);
    insert_with_values(store, Some(&access), None, &[0, 1], &[&"private", &format!("{}", (access_flags & methods::AccessFlags::Private as u16) != 0)]);
    insert_with_values(store, Some(&access), None, &[0, 1], &[&"protected", &format!("{}", (access_flags & methods::AccessFlags::Protected as u16) != 0)]);
    insert_with_values(store, Some(&access), None, &[0, 1], &[&"static", &format!("{}", (access_flags & methods::AccessFlags::Static as u16) != 0)]);
    insert_with_values(store, Some(&access), None, &[0, 1], &[&"final", &format!("{}", (access_flags & methods::AccessFlags::Final as u16) != 0)]);
    insert_with_values(store, Some(&access), None, &[0, 1], &[&"synchronized", &format!("{}", (access_flags & methods::AccessFlags::Synchronized as u16) != 0)]);
    insert_with_values(store, Some(&access), None, &[0, 1], &[&"bridge", &format!("{}", (access_flags & methods::AccessFlags::Bridge as u16) != 0)]);
    insert_with_values(store, Some(&access), None, &[0, 1], &[&"varargs", &format!("{}", (access_flags & methods::AccessFlags::Varargs as u16) != 0)]);
    insert_with_values(store, Some(&access), None, &[0, 1], &[&"native", &format!("{}", (access_flags & methods::AccessFlags::Native as u16) != 0)]);
    insert_with_values(store, Some(&access), None, &[0, 1], &[&"abstract", &format!("{}", (access_flags & methods::AccessFlags::Abstract as u16) != 0)]);
    insert_with_values(store, Some(&access), None, &[0, 1], &[&"strict", &format!("{}", (access_flags & methods::AccessFlags::Strict as u16) != 0)]);
    insert_with_values(store, Some(&access), None, &[0, 1], &[&"synthetic", &format!("{}", (access_flags & methods::AccessFlags::Synthetic as u16) != 0)]);
}

fn insert_access_method_param(store: &TreeStore, iter: &TreeIter, access_flags: u16) {
    let access = insert_with_values(store, Some(&iter), None, &[0, 1], &[&"access_flags", &format!("{:#06X}", access_flags)]);
    insert_with_values(store, Some(&access), None, &[0, 1], &[&"final", &format!("{}", (access_flags & attributes::MethodParameterAccessFlags::Final as u16) != 0)]);
    insert_with_values(store, Some(&access), None, &[0, 1], &[&"synthetic", &format!("{}", (access_flags & attributes::MethodParameterAccessFlags::Synthetic as u16) != 0)]);
    insert_with_values(store, Some(&access), None, &[0, 1], &[&"mandated", &format!("{}", (access_flags & attributes::MethodParameterAccessFlags::Mandated as u16) != 0)]);
}

fn insert_attributes(cp: &ConstantPool, store: &TreeStore, iter: &TreeIter, attributes: Vec<Attribute>) {
    let iter_a = insert_with_values(store, Some(&iter), None, &[0, 1], &[&"Attributes", &""]);
    for attr in attributes {
        match attr {
            ConstantValue { constantvalue_index } => {
                let iter_b = insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&"ConstantValue", &""]);
                insert_with_values(store, Some(&iter_b), None, &[0, 1], &[&"constantvalue_index", &format!("{}", constantvalue_index)]);
            }
            Code { max_stack, max_locals, code, exception_table, attributes } => {
                let iter_b = insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&"Code", &""]);
                insert_with_values(store, Some(&iter_b), None, &[0, 1], &[&"max_stack", &format!("{}", max_stack)]);
                insert_with_values(store, Some(&iter_b), None, &[0, 1], &[&"max_locals", &format!("{}", max_locals)]);
                let mut pos = 0;
                insert_code(store, &iter_b, code, cp, &mut pos);
                let iter_c = insert_with_values(store, Some(&iter_b), None, &[0, 1], &[&"exception_table", &""]);
                for (i, e) in exception_table.iter().enumerate() {
                    let iter_d = insert_with_values(store, Some(&iter_c), None, &[0, 1], &[&format!("Entry {}", i), &""]);
                    insert_with_values(store, Some(&iter_d), None, &[0, 1], &[&"start_pc", &format!("{}", e.start_pc)]);
                    insert_with_values(store, Some(&iter_d), None, &[0, 1], &[&"end_pc", &format!("{}", e.end_pc)]);
                    insert_with_values(store, Some(&iter_d), None, &[0, 1], &[&"handler_pc", &format!("{}", e.handler_pc)]);
                    insert_with_values(store, Some(&iter_d), None, &[0, 1], &[&"catch_type", &format!("{}", e.catch_type)]);
                }
                insert_attributes(cp, store, &iter_b, attributes);
            }
            StackMapTable { entries } => {
                let iter_ba = insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&"StackMapTable", &""]);
                let iter_b = insert_with_values(store, Some(&iter_ba), None, &[0, 1], &[&"Entries", &""]);
                let mut index: i32 = -1;
                for entry in entries {
                    match entry {
                        StackMapFrame::SameFrame { offset_delta } => {
                            index += offset_delta as i32 + 1;
                            let iter_c = insert_with_values(store, Some(&iter_b), None, &[0, 1], &[&"SameFrame", &format!("{}", index)]);
                            insert_with_values(store, Some(&iter_c), None, &[0, 1], &[&"offset_delta", &format!("{}", offset_delta)]);
                        }
                        StackMapFrame::SameLocals1Item { offset_delta, stack } => {
                            index += offset_delta as i32 + 1;
                            let iter_c = insert_with_values(store, Some(&iter_b), None, &[0, 1], &[&"SameLocals1Item", &format!("{}", index)]);
                            insert_with_values(store, Some(&iter_c), None, &[0, 1], &[&"offset_delta", &format!("{}", offset_delta)]);
                            insert_vti(store, &iter_c, stack, cp);
                        }
                        StackMapFrame::SameLocals1ItemExtended { offset_delta, stack } => {
                            index += offset_delta as i32 + 1;
                            let iter_c = insert_with_values(store, Some(&iter_b), None, &[0, 1], &[&"SameLocals1ItemExtended", &format!("{}", index)]);
                            insert_with_values(store, Some(&iter_c), None, &[0, 1], &[&"offset_delta", &format!("{}", offset_delta)]);
                            insert_vti(store, &iter_c, stack, cp);
                        }
                        StackMapFrame::ChopFrame { absent_locals, offset_delta } => {
                            index += offset_delta as i32 + 1;
                            let iter_c = insert_with_values(store, Some(&iter_b), None, &[0, 1], &[&"ChopFrame", &format!("{}", index)]);
                            insert_with_values(store, Some(&iter_c), None, &[0, 1], &[&"offset_delta", &format!("{}", offset_delta)]);
                            insert_with_values(store, Some(&iter_c), None, &[0, 1], &[&"absent_locals", &format!("{}", absent_locals)]);
                        }
                        StackMapFrame::SameFrameExtended { offset_delta } => {
                            index += offset_delta as i32 + 1;
                            let iter_c = insert_with_values(store, Some(&iter_b), None, &[0, 1], &[&"SameFrameExtended", &format!("{}", index)]);
                            insert_with_values(store, Some(&iter_c), None, &[0, 1], &[&"offset_delta", &format!("{}", offset_delta)]);
                        }
                        StackMapFrame::AppendFrame { offset_delta, locals } => {
                            index += offset_delta as i32 + 1;
                            let iter_c = insert_with_values(store, Some(&iter_b), None, &[0, 1], &[&"SameLocals1ItemExtended", &format!("{}", index)]);
                            insert_with_values(store, Some(&iter_c), None, &[0, 1], &[&"offset_delta", &format!("{}", offset_delta)]);
                            let iter_d = insert_with_values(store, Some(&iter_c), None, &[0, 1], &[&"locals", &""]);
                            for vti in locals {
                                insert_vti(store, &iter_d, vti, cp);
                            }
                        }
                        StackMapFrame::FullFrame { offset_delta, locals, stack } => {
                            index += offset_delta as i32 + 1;
                            let iter_c = insert_with_values(store, Some(&iter_b), None, &[0, 1], &[&"FullFrame", &format!("{}", index)]);
                            insert_with_values(store, Some(&iter_c), None, &[0, 1], &[&"offset_delta", &format!("{}", offset_delta)]);
                            let iter_d = insert_with_values(store, Some(&iter_c), None, &[0, 1], &[&"locals", &""]);
                            for vti in locals {
                                insert_vti(store, &iter_d, vti, cp);
                            }
                            let iter_d = insert_with_values(store, Some(&iter_c), None, &[0, 1], &[&"stack", &""]);
                            for vti in stack {
                                insert_vti(store, &iter_d, vti, cp);
                            }
                        }
                    }
                }
            }
            Exceptions { exception_index_table } => {
                let iter_b = insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&"Exceptions", &""]);
                let iter_c = insert_with_values(store, Some(&iter_b), None, &[0, 1], &[&"exception_index_table", &""]);
                for i in exception_index_table {
                    insert_with_values(store, Some(&iter_c), None, &[0, 1], &[&format!("{}", i), &""]);
                }
            }
            InnerClasses { classes } => {
                let iter_b = insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&"InnerClasses", &""]);
                for class in classes {
                    let iter_c = insert_with_values(store, Some(&iter_b), None, &[0, 1], &[&"InnerClass", &if class.inner_name_index.as_u16() == 0 { "Anonymous class".to_owned() } else { get_name(cp, class.inner_name_index) }]);
                    insert_with_values(store, Some(&iter_c), None, &[0, 1], &[&"inner_java_class_index", &format!("{}", class.inner_class_info_index)]);
                    insert_with_values(store, Some(&iter_c), None, &[0, 1], &[&"outer_java_class_index", &format!("{}", class.outer_class_info_index)]);
                    insert_with_values(store, Some(&iter_c), None, &[0, 1], &[&"inner_name_index", &format!("{}", class.inner_name_index)]);
                    insert_access_class(store, &iter_c, class.inner_class_access_flags);
                }
            }
            EnclosingMethod { class_index, method_index } => {
                let iter_b = insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&"EnclosingMethod", &""]);
                insert_with_values(store, Some(&iter_b), None, &[0, 1], &[&"class_index", &format!("{}", class_index)]);
                insert_with_values(store, Some(&iter_b), None, &[0, 1], &[&"method_index", &format!("{}", method_index)]);
            }
            Synthetic => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&"Synthetic", &""]);
            }
            Signature { signature_index } => {
                let iter_b = insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&"Signature", &""]);
                insert_with_values(store, Some(&iter_b), None, &[0, 1], &[&"signature_index", &format!("{}", signature_index)]);
            }
            SourceFile { sourcefile_index } => {
                let iter_b = insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&"sourcefile_index", &""]);
                insert_with_values(store, Some(&iter_b), None, &[0, 1], &[&"sourcefile_index", &format!("{}", sourcefile_index)]);
            }
            SourceDebugExtenson { debug_extension } => {
                let iter_b = insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&"SourceDebugExtension", &""]);
                let iter_c = insert_with_values(store, Some(&iter_b), None, &[0, 1], &[&"debug_extension", &""]);
                for i in debug_extension {
                    insert_with_values(store, Some(&iter_c), None, &[0, 1], &[&format!("{}", i), &""]);
                }
            }
            LineNumberTable { line_number_table } => {
                let iter_b = insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&"LineNumberTable", &""]);
                for t in line_number_table {
                    let iter_c = insert_with_values(store, Some(&iter_b), None, &[0, 1], &[&"Entry", &""]);
                    insert_with_values(store, Some(&iter_c), None, &[0, 1], &[&"start_pc", &format!("{}", t.start_pc)]);
                    insert_with_values(store, Some(&iter_c), None, &[0, 1], &[&"line_number", &format!("{}", t.line_number)]);
                }
            }
            LocalVariableTable { local_variable_table } => {
                let iter_b = insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&"LocalVariableTable", &""]);
                for lv in local_variable_table {
                    let iter_c = insert_with_values(store, Some(&iter_b), None, &[0, 1], &[&"Entry", &format!("{} {}", get_name(cp, lv.descriptor_index), get_name(cp, lv.name_index))]);
                    insert_with_values(store, Some(&iter_c), None, &[0, 1], &[&"start_pc", &format!("{}", lv.start_pc)]);
                    insert_with_values(store, Some(&iter_c), None, &[0, 1], &[&"length", &format!("{}", lv.length)]);
                    insert_with_values(store, Some(&iter_c), None, &[0, 1], &[&"name_index", &format!("{}", lv.name_index)]);
                    insert_with_values(store, Some(&iter_c), None, &[0, 1], &[&"descriptor_index", &format!("{}", lv.descriptor_index)]);
                    insert_with_values(store, Some(&iter_c), None, &[0, 1], &[&"index", &format!("{}", lv.index)]);
                }
            }
            LocalVariableTypeTable { local_variable_type_table } => {
                let iter_b = insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&"LocalVariableTypeTable", &""]);
                for lv in local_variable_type_table {
                    let iter_c = insert_with_values(store, Some(&iter_b), None, &[0, 1], &[&"Entry", &""]);
                    insert_with_values(store, Some(&iter_c), None, &[0, 1], &[&"start_pc", &format!("{}", lv.start_pc)]);
                    insert_with_values(store, Some(&iter_c), None, &[0, 1], &[&"length", &format!("{}", lv.length)]);
                    insert_with_values(store, Some(&iter_c), None, &[0, 1], &[&"name_index", &format!("{}", lv.name_index)]);
                    insert_with_values(store, Some(&iter_c), None, &[0, 1], &[&"signature_index", &format!("{}", lv.signature_index)]);
                    insert_with_values(store, Some(&iter_c), None, &[0, 1], &[&"index", &format!("{}", lv.index)]);
                }
            }
            Deprecated => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&"Deprecated", &""]);
            }
            RuntimeVisibleAnnotations { annotations } => {
                let iter_b = insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&"RuntimeVisibleAnnotations", &""]);
                insert_annotations(store, &iter_b, annotations);
            }
            RuntimeInvisibleAnnotations { annotations } => {
                let iter_b = insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&"RuntimeInvisibleAnnotations", &""]);
                insert_annotations(store, &iter_b, annotations);
            }
            RuntimeVisibleParameterAnnotations { parameter_annotations } => {
                let iter_b = insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&"RuntimeVisibleParameterAnnotations", &""]);
                for (i, e) in parameter_annotations.iter().enumerate() {
                    let iter_c = insert_with_values(store, Some(&iter_b), None, &[0, 1], &[&format!("{}", i), &""]);
                    for a in e {
                        insert_annotation(store, &iter_c, a);
                    }
                }
            }
            RuntimeInvisibleParameterAnnotations { parameter_annotations } => {
                let iter_b = insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&"RuntimeInvisibleParameterAnnotations", &""]);
                for (i, e) in parameter_annotations.iter().enumerate() {
                    let iter_c = insert_with_values(store, Some(&iter_b), None, &[0, 1], &[&format!("{}", i), &""]);
                    for a in e {
                        insert_annotation(store, &iter_c, a);
                    }
                }
            }
            RuntimeVisibleTypeAnnotations { annotations } => {
                let iter_b = insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&"RuntimeVisibleTypeAnnotations", &""]);
                insert_type_annotations(store, &iter_b, annotations);
            }
            RuntimeInvisibleTypeAnnotations { annotations } => {
                let iter_b = insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&"RuntimeInvisibleTypeAnnotations", &""]);
                insert_type_annotations(store, &iter_b, annotations);
            }
            AnnotationDefault { default_value } => {
                let iter_b = insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&"AnnotationDefault", &""]);
                let iter_c = insert_with_values(store, Some(&iter_b), None, &[0, 1], &[&"default_value", &""]);
                insert_element_value(store, &iter_c, &default_value);
            }
            BootstrapMethods { bootstrap_methods } => {
                let iter_b = insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&"BootstrapMethods", &""]);
                for m in bootstrap_methods {
                    let iter_c = insert_with_values(store, Some(&iter_b), None, &[0, 1], &[&"Entry", &""]);
                    insert_with_values(store, Some(&iter_c), None, &[0, 1], &[&"bootstrap_method_ref", &format!("{}", m.bootstrap_method_ref)]);
                    let iter_d = insert_with_values(store, Some(&iter_c), None, &[0, 1], &[&"bootstrap_arguments", &""]);
                    for i in 0..m.bootstrap_arguments.len() {
                        insert_with_values(store, Some(&iter_d), None, &[0, 1], &[&format!("{}", i), &format!("{}", m.bootstrap_arguments[i])]);
                    }
                }
            }
            MethodParameters { parameters } => {
                let iter_b = insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&"MethodParameters", &""]);
                for p in parameters {
                    let iter_c = insert_with_values(store, Some(&iter_b), None, &[0, 1], &[&"Entry", &get_name(cp, p.name_index)]);
                    insert_with_values(store, Some(&iter_c), None, &[0, 1], &[&"name_index", &format!("{}", p.name_index)]);
                    insert_access_method_param(store, &iter_c, p.access_flags);
                }
            }
        }
    }
}

fn insert_type_annotations(store: &TreeStore, iter: &TreeIter, annotations: Vec<TypeAnnotation>) {
    let iter_a = insert_with_values(store, Some(iter), None, &[0, 1], &[&"Annotations", &""]);
    for a in annotations {
        let iter_b = insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&"TypeAnnotation", &""]);
        match a.target_info {
            TargetInfo::TypeParameterTarget { type_parameter_index } => {
                let iter_c = insert_with_values(store, Some(&iter_b), None, &[0, 1], &[&"TypeParameterTarget", &""]);
                insert_with_values(store, Some(&iter_c), None, &[0, 1], &[&"type_parameter_index", &format!("{}", type_parameter_index)]);
            }
            TargetInfo::SupertypeTarget { supertype_index } => {
                let iter_c = insert_with_values(store, Some(&iter_b), None, &[0, 1], &[&"SupertypeTarget", &""]);
                insert_with_values(store, Some(&iter_c), None, &[0, 1], &[&"supertype_index", &format!("{}", supertype_index)]);
            }
            TargetInfo::TypeParameterBoundTarget { type_parameter_index, bound_index } => {
                let iter_c = insert_with_values(store, Some(&iter_b), None, &[0, 1], &[&"TypeParameterTarget", &""]);
                insert_with_values(store, Some(&iter_c), None, &[0, 1], &[&"type_parameter_index", &format!("{}", type_parameter_index)]);
                insert_with_values(store, Some(&iter_c), None, &[0, 1], &[&"bound_index", &format!("{}", bound_index)]);
            }
            TargetInfo::EmptyTarget => {
                insert_with_values(store, Some(&iter_b), None, &[0, 1], &[&"EmptyTarget", &""]);
            }
            TargetInfo::FormalParameterTarget { formal_parameter_index } => {
                let iter_c = insert_with_values(store, Some(&iter_b), None, &[0, 1], &[&"FormalParameterTarget", &""]);
                insert_with_values(store, Some(&iter_c), None, &[0, 1], &[&"formal_parameter_index", &format!("{}", formal_parameter_index)]);
            }
            TargetInfo::ThrowsTarget { throws_type_index } => {
                let iter_c = insert_with_values(store, Some(&iter_b), None, &[0, 1], &[&"ThrowsTarget", &""]);
                insert_with_values(store, Some(&iter_c), None, &[0, 1], &[&"throws_type_index", &format!("{}", throws_type_index)]);
            }
            TargetInfo::LocalVarTarget { table } => {
                let iter_c = insert_with_values(store, Some(&iter_b), None, &[0, 1], &[&"LocalVarTarget", &""]);
                let iter_d = insert_with_values(store, Some(&iter_c), None, &[0, 1], &[&"table", &""]);
                for te in table {
                    insert_with_values(store, Some(&iter_d), None, &[0, 1], &[&"start_pc", &format!("{}", te.start_pc)]);
                    insert_with_values(store, Some(&iter_d), None, &[0, 1], &[&"length", &format!("{}", te.length)]);
                    insert_with_values(store, Some(&iter_d), None, &[0, 1], &[&"index", &format!("{}", te.index)]);
                }
            }
            TargetInfo::CatchTarget { exception_table_index } => {
                let iter_c = insert_with_values(store, Some(&iter_b), None, &[0, 1], &[&"CatchTarget", &""]);
                insert_with_values(store, Some(&iter_c), None, &[0, 1], &[&"exception_table_index", &format!("{}", exception_table_index)]);
            }
            TargetInfo::OffsetTarget { offset } => {
                let iter_c = insert_with_values(store, Some(&iter_b), None, &[0, 1], &[&"OffsetTarget", &""]);
                insert_with_values(store, Some(&iter_c), None, &[0, 1], &[&"offset", &format!("{}", offset)]);
            }
            TargetInfo::TypeArgumentTarget { offset, type_argument_index } => {
                let iter_c = insert_with_values(store, Some(&iter_b), None, &[0, 1], &[&"TypeArgumentTarget", &""]);
                insert_with_values(store, Some(&iter_c), None, &[0, 1], &[&"offset", &format!("{}", offset)]);
                insert_with_values(store, Some(&iter_c), None, &[0, 1], &[&"type_argument_index", &format!("{}", type_argument_index)]);
            }
        }
    }
}

fn insert_annotations(store: &TreeStore, iter: &TreeIter, annotations: Vec<Annotation>) {
    let iter_a = insert_with_values(store, Some(iter), None, &[0, 1], &[&"Annotations", &""]);
    for a in annotations {
        insert_annotation(store, &iter_a, &a);
    }
}

fn insert_annotation(store: &TreeStore, iter: &TreeIter, annotation: &Annotation) {
    let iter_b = insert_with_values(store, Some(iter), None, &[0, 1], &[&"Annotation", &""]);
    insert_with_values(store, Some(&iter_b), None, &[0, 1], &[&"type_index", &format!("{}", annotation.type_index)]);
    let iter_c = insert_with_values(store, Some(&iter_b), None, &[0, 1], &[&"element_value_pairs", &""]);
    for pair in &annotation.element_value_pairs {
        let iter_d = insert_with_values(store, Some(&iter_c), None, &[0, 1], &[&"Pair", &""]);
        insert_with_values(store, Some(&iter_d), None, &[0, 1], &[&"element_name_index", &format!("{}", pair.element_name_index)]);
        insert_element_value(store, &iter_d, &pair.value);
    }
}

fn insert_element_value(store: &TreeStore, iter: &TreeIter, val: &ElementValue) {
    match val {
        ElementValue::ConstValueIndex(a) => {
            insert_with_values(store, Some(iter), None, &[0, 1], &[&"ConstValueIndex", &format!("{}", a)]);
        }
        ElementValue::EnumConstValue { type_name_index, const_name_index } => {
            let iter_a = insert_with_values(store, Some(iter), None, &[0, 1], &[&"EnumConstValue", &""]);
            insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&"type_name_index", &format!("{}", type_name_index)]);
            insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&"const_name_index", &format!("{}", const_name_index)]);
        }
        ElementValue::ClassInfoIndex(a) => {
            insert_with_values(store, Some(iter), None, &[0, 1], &[&"ClassInfoIndex", &format!("{}", a)]);
        }
        ElementValue::AnnotationValue(a) => {
            let iter_a = insert_with_values(store, Some(iter), None, &[0, 1], &[&"AnnotationValue", &""]);
            insert_annotation(store, &iter_a, &a);
        }
        ElementValue::ArrayValue(vs) => {
            let iter_a = insert_with_values(store, Some(iter), None, &[0, 1], &[&"ArrayValue", &""]);
            for v in vs {
                insert_element_value(store, &iter_a, v);
            }
        }
    }
}

fn insert_vti(store: &TreeStore, iter: &TreeIter, vti: VerificationTypeInfo, cp: &ConstantPool) {
    match vti {
        VerificationTypeInfo::Top => {
            insert_with_values(store, Some(&iter), None, &[0, 1], &[&"VerificationTypeInfo", &"Top"]);
        }
        VerificationTypeInfo::Integer => {
            insert_with_values(store, Some(&iter), None, &[0, 1], &[&"VerificationTypeInfo", &"Integer"]);
        }
        VerificationTypeInfo::Float => {
            insert_with_values(store, Some(&iter), None, &[0, 1], &[&"VerificationTypeInfo", &"Float"]);
        }
        VerificationTypeInfo::Null => {
            insert_with_values(store, Some(&iter), None, &[0, 1], &[&"VerificationTypeInfo", &"Null"]);
        }
        VerificationTypeInfo::UninitializedThis => {
            insert_with_values(store, Some(&iter), None, &[0, 1], &[&"VerificationTypeInfo", &"UninitializedThis"]);
        }
        VerificationTypeInfo::Object { cpool_index } => {
            let iter_a = insert_with_values(store, Some(&iter), None, &[0, 1], &[&"VerificationTypeInfo", &format!("Object ({})", get_name(cp, cpool_index))]);
            insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&"cpool_index", &format!("{}", cpool_index)]);
        }
        VerificationTypeInfo::UninitializedVariable { offset } => {
            let iter_a = insert_with_values(store, Some(&iter), None, &[0, 1], &[&"VerificationTypeInfo", &"UninitializedVariable"]);
            insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&"cpool_index", &format!("{}", offset)]);
        }
        VerificationTypeInfo::Long => {
            insert_with_values(store, Some(&iter), None, &[0, 1], &[&"VerificationTypeInfo", &"Long"]);
        }
        VerificationTypeInfo::Double => {
            insert_with_values(store, Some(&iter), None, &[0, 1], &[&"VerificationTypeInfo", &"Double"]);
        }
    }
}

fn insert_code(store: &TreeStore, iter: &TreeIter, code: Vec<Opcode>, cp: &ConstantPool, pos: &mut u32) {
    let iter_a = insert_with_values(store, Some(&iter), None, &[0, 1], &[&"Code", &""]);
    for op in code {
        match op {
            aaload => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. aaload", pos), &""]);
                *pos += 1;
            }
            aastore => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. aastore", pos), &""]);
                *pos += 1;
            }
            aconst_null => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. aconst_null", pos), &""]);
                *pos += 1;
            }
            aload { index } => {
                let iter_b = insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. aload", pos), &""]);
                insert_with_values(store, Some(&iter_b), None, &[0, 1], &[&"index", &format!("{}", index)]);
                *pos += 2;
            }
            aload_0 => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. aload_0", pos), &""]);
                *pos += 1;
            }
            aload_1 => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. aload_1", pos), &""]);
                *pos += 1;
            }
            aload_2 => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. aload_2", pos), &""]);
                *pos += 1;
            }
            aload_3 => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. aload_3", pos), &""]);
                *pos += 1;
            }
            anewarray { index } => {
                let iter_b = insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. anewarray", pos), &get_name(cp, index)]);
                insert_with_values(store, Some(&iter_b), None, &[0, 1], &[&"index", &format!("{}", index)]);
                *pos += 3;
            }
            areturn => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. areturn", pos), &""]);
                *pos += 1;
            }
            arraylength => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. arraylength", pos), &""]);
                *pos += 1;
            }
            astore { index } => {
                let iter_b = insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. astore", pos), &""]);
                insert_with_values(store, Some(&iter_b), None, &[0, 1], &[&"index", &format!("{}", index)]);
                *pos += 2;
            }
            astore_0 => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. astore_0", pos), &""]);
                *pos += 1;
            }
            astore_1 => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. astore_1", pos), &""]);
                *pos += 1;
            }
            astore_2 => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. astore_2", pos), &""]);
                *pos += 1;
            }
            astore_3 => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. astore_3", pos), &""]);
                *pos += 1;
            }
            athrow => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. athrow", pos), &""]);
                *pos += 1;
            }
            baload => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. baload", pos), &""]);
                *pos += 1;
            }
            bastore => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. bastore", pos), &""]);
                *pos += 1;
            }
            bipush { val } => {
                let iter_b = insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. bipush", pos), &""]);
                insert_with_values(store, Some(&iter_b), None, &[0, 1], &[&"val", &format!("{}", val)]);
                *pos += 2;
            }
            breakpoint => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. breakpoint", pos), &""]);
                *pos += 1;
            }
            caload => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. caload", pos), &""]);
                *pos += 1;
            }
            castore => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. castore", pos), &""]);
                *pos += 1;
            }
            checkcast { index } => {
                let iter_b = insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. checkcast", pos), &get_name(cp, index)]);
                insert_with_values(store, Some(&iter_b), None, &[0, 1], &[&"index", &format!("{}", index)]);
                *pos += 3;
            }
            d2f => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. d2f", pos), &""]);
                *pos += 1;
            }
            d2i => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. d2i", pos), &""]);
                *pos += 1;
            }
            d2l => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. d2l", pos), &""]);
                *pos += 1;
            }
            dadd => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. dadd", pos), &""]);
                *pos += 1;
            }
            daload => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. daload", pos), &""]);
                *pos += 1;
            }
            dastore => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. dastore", pos), &""]);
                *pos += 1;
            }
            dcmpg => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. dcmpg", pos), &""]);
                *pos += 1;
            }
            dcmpl => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. dcmpl", pos), &""]);
                *pos += 1;
            }
            dconst_0 => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. dconst_0", pos), &""]);
                *pos += 1;
            }
            dconst_1 => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. dconst_1", pos), &""]);
                *pos += 1;
            }
            ddiv => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. ddiv", pos), &""]);
                *pos += 1;
            }
            dload { index } => {
                let iter_b = insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. dload", pos), &""]);
                insert_with_values(store, Some(&iter_b), None, &[0, 1], &[&"index", &format!("{}", index)]);
                *pos += 2;
            }
            dload_0 => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. dload_0", pos), &""]);
                *pos += 1;
            }
            dload_1 => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. dload_1", pos), &""]);
                *pos += 1;
            }
            dload_2 => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. dload_2", pos), &""]);
                *pos += 1;
            }
            dload_3 => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. dload_3", pos), &""]);
                *pos += 1;
            }
            dmul => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. dmul", pos), &""]);
                *pos += 1;
            }
            dneg => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. dneg", pos), &""]);
                *pos += 1;
            }
            drem => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. drem", pos), &""]);
                *pos += 1;
            }
            dreturn => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. dreturn", pos), &""]);
                *pos += 1;
            }
            dstore { index } => {
                let iter_b = insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. dstore", pos), &""]);
                insert_with_values(store, Some(&iter_b), None, &[0, 1], &[&"index", &format!("{}", index)]);
                *pos += 2;
            }
            dstore_0 => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. dstore_0", pos), &""]);
                *pos += 1;
            }
            dstore_1 => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. dstore_1", pos), &""]);
                *pos += 1;
            }
            dstore_2 => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. dstore_2", pos), &""]);
                *pos += 1;
            }
            dstore_3 => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. dstore_3", pos), &""]);
                *pos += 1;
            }
            dsub => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. dsub", pos), &""]);
                *pos += 1;
            }
            dup => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. dup", pos), &""]);
                *pos += 1;
            }
            dup_x1 => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. dup_x1", pos), &""]);
                *pos += 1;
            }
            dup_x2 => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. dup_x2", pos), &""]);
                *pos += 1;
            }
            dup2 => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. dup2", pos), &""]);
                *pos += 1;
            }
            dup2_x1 => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. dup2_x1", pos), &""]);
                *pos += 1;
            }
            dup2_x2 => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. dup2_x2", pos), &""]);
                *pos += 1;
            }
            f2d => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. f2d", pos), &""]);
                *pos += 1;
            }
            f2i => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. f2i", pos), &""]);
                *pos += 1;
            }
            f2l => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. f2l", pos), &""]);
                *pos += 1;
            }
            fadd => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. fadd", pos), &""]);
                *pos += 1;
            }
            faload => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. faload", pos), &""]);
                *pos += 1;
            }
            fastore => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. fastore", pos), &""]);
                *pos += 1;
            }
            fcmpg => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. fcmpg", pos), &""]);
                *pos += 1;
            }
            fcmpl => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. fcmpl", pos), &""]);
                *pos += 1;
            }
            fconst_0 => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. fconst_0", pos), &""]);
                *pos += 1;
            }
            fconst_1 => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. fconst_1", pos), &""]);
                *pos += 1;
            }
            fconst_2 => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. fconst_2", pos), &""]);
                *pos += 1;
            }
            fdiv => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. fdiv", pos), &""]);
                *pos += 1;
            }
            fload { index } => {
                let iter_b = insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. fload", pos), &""]);
                insert_with_values(store, Some(&iter_b), None, &[0, 1], &[&"index", &format!("{}", index)]);
                *pos += 2;
            }
            fload_0 => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. fload_0", pos), &""]);
                *pos += 1;
            }
            fload_1 => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. fload_1", pos), &""]);
                *pos += 1;
            }
            fload_2 => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. fload_2", pos), &""]);
                *pos += 1;
            }
            fload_3 => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. fload_3", pos), &""]);
                *pos += 1;
            }
            fmul => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. fmul", pos), &""]);
                *pos += 1;
            }
            fneg => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. fneg", pos), &""]);
                *pos += 1;
            }
            frem => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. frem", pos), &""]);
                *pos += 1;
            }
            freturn => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. freturn", pos), &""]);
                *pos += 1;
            }
            fstore { index } => {
                let iter_b = insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. fstore", pos), &""]);
                insert_with_values(store, Some(&iter_b), None, &[0, 1], &[&"index", &format!("{}", index)]);
                *pos += 2;
            }
            fstore_0 => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. fstore_0", pos), &""]);
                *pos += 1;
            }
            fstore_1 => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. fstore_1", pos), &""]);
                *pos += 1;
            }
            fstore_2 => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. fstore_2", pos), &""]);
                *pos += 1;
            }
            fstore_3 => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. fstore_3", pos), &""]);
                *pos += 1;
            }
            fsub => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. fsub", pos), &""]);
                *pos += 1;
            }
            getfield { index } => {
                let iter_b = insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. getfield", pos), &get_name(cp, index)]);
                insert_with_values(store, Some(&iter_b), None, &[0, 1], &[&"index", &format!("{}", index)]);
                *pos += 3;
            }
            getstatic { index } => {
                let iter_b = insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. getstatic", pos), &get_name(cp, index)]);
                insert_with_values(store, Some(&iter_b), None, &[0, 1], &[&"index", &format!("{}", index)]);
                *pos += 3;
            }
            goto { branch } => {
                let iter_b = insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. goto", pos), &""]);
                insert_with_values(store, Some(&iter_b), None, &[0, 1], &[&"branch", &format!("{} ({})", branch, branch as i32 + *pos as i32)]);
                *pos += 3;
            }
            goto_w { branch } => {
                let iter_b = insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. goto_w", pos), &""]);
                insert_with_values(store, Some(&iter_b), None, &[0, 1], &[&"branch", &format!("{} ({})", branch, branch as i32 + *pos as i32)]);
                *pos += 5;
            }
            i2b => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. i2b", pos), &""]);
                *pos += 1;
            }
            i2c => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. i2c", pos), &""]);
                *pos += 1;
            }
            i2d => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. i2d", pos), &""]);
                *pos += 1;
            }
            i2f => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. i2f", pos), &""]);
                *pos += 1;
            }
            i2l => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. i2l", pos), &""]);
                *pos += 1;
            }
            i2s => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. i2s", pos), &""]);
                *pos += 1;
            }
            iadd => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. iadd", pos), &""]);
                *pos += 1;
            }
            iaload => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. iaload", pos), &""]);
                *pos += 1;
            }
            iand => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. iand", pos), &""]);
                *pos += 1;
            }
            iastore => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. iastore", pos), &""]);
                *pos += 1;
            }
            iconst_m1 => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. iconst_m1", pos), &""]);
                *pos += 1;
            }
            iconst_0 => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. iconst_0", pos), &""]);
                *pos += 1;
            }
            iconst_1 => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. iconst_1", pos), &""]);
                *pos += 1;
            }
            iconst_2 => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. iconst_2", pos), &""]);
                *pos += 1;
            }
            iconst_3 => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. iconst_3", pos), &""]);
                *pos += 1;
            }
            iconst_4 => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. iconst_4", pos), &""]);
                *pos += 1;
            }
            iconst_5 => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. iconst_5", pos), &""]);
                *pos += 1;
            }
            idiv => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. idiv", pos), &""]);
                *pos += 1;
            }
            if_acmpeq { branch } => {
                let iter_b = insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. if_acmpeq", pos), &""]);
                insert_with_values(store, Some(&iter_b), None, &[0, 1], &[&"branch", &format!("{} ({})", branch, branch as i32 + *pos as i32)]);
                *pos += 3;
            }
            if_acmpne { branch } => {
                let iter_b = insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. if_acmpne", pos), &""]);
                insert_with_values(store, Some(&iter_b), None, &[0, 1], &[&"branch", &format!("{} ({})", branch, branch as i32 + *pos as i32)]);
                *pos += 3;
            }
            if_icmpeq { branch } => {
                let iter_b = insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. if_icmpeq", pos), &""]);
                insert_with_values(store, Some(&iter_b), None, &[0, 1], &[&"branch", &format!("{} ({})", branch, branch as i32 + *pos as i32)]);
                *pos += 3;
            }
            if_icmpge { branch } => {
                let iter_b = insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. if_icmpge", pos), &""]);
                insert_with_values(store, Some(&iter_b), None, &[0, 1], &[&"branch", &format!("{} ({})", branch, branch as i32 + *pos as i32)]);
                *pos += 3;
            }
            if_icmpgt { branch } => {
                let iter_b = insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. if_icmpgt", pos), &""]);
                insert_with_values(store, Some(&iter_b), None, &[0, 1], &[&"branch", &format!("{} ({})", branch, branch as i32 + *pos as i32)]);
                *pos += 3;
            }
            if_icmple { branch } => {
                let iter_b = insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. if_icmple", pos), &""]);
                insert_with_values(store, Some(&iter_b), None, &[0, 1], &[&"branch", &format!("{} ({})", branch, branch as i32 + *pos as i32)]);
                *pos += 3;
            }
            if_icmplt { branch } => {
                let iter_b = insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. if_icmplt", pos), &""]);
                insert_with_values(store, Some(&iter_b), None, &[0, 1], &[&"branch", &format!("{} ({})", branch, branch as i32 + *pos as i32)]);
                *pos += 3;
            }
            if_icmpne { branch } => {
                let iter_b = insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. if_icmpne", pos), &""]);
                insert_with_values(store, Some(&iter_b), None, &[0, 1], &[&"branch", &format!("{} ({})", branch, branch as i32 + *pos as i32)]);
                *pos += 3;
            }
            ifeq { branch } => {
                let iter_b = insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. ifeq", pos), &""]);
                insert_with_values(store, Some(&iter_b), None, &[0, 1], &[&"branch", &format!("{} ({})", branch, branch as i32 + *pos as i32)]);
                *pos += 3;
            }
            ifge { branch } => {
                let iter_b = insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. ifge", pos), &""]);
                insert_with_values(store, Some(&iter_b), None, &[0, 1], &[&"branch", &format!("{} ({})", branch, branch as i32 + *pos as i32)]);
                *pos += 3;
            }
            ifgt { branch } => {
                let iter_b = insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. ifgt", pos), &""]);
                insert_with_values(store, Some(&iter_b), None, &[0, 1], &[&"branch", &format!("{} ({})", branch, branch as i32 + *pos as i32)]);
                *pos += 3;
            }
            ifle { branch } => {
                let iter_b = insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. ifle", pos), &""]);
                insert_with_values(store, Some(&iter_b), None, &[0, 1], &[&"branch", &format!("{} ({})", branch, branch as i32 + *pos as i32)]);
                *pos += 3;
            }
            iflt { branch } => {
                let iter_b = insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. iflt", pos), &""]);
                insert_with_values(store, Some(&iter_b), None, &[0, 1], &[&"branch", &format!("{} ({})", branch, branch as i32 + *pos as i32)]);
                *pos += 3;
            }
            ifne { branch } => {
                let iter_b = insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. ifne", pos), &""]);
                insert_with_values(store, Some(&iter_b), None, &[0, 1], &[&"branch", &format!("{} ({})", branch, branch as i32 + *pos as i32)]);
                *pos += 3;
            }
            ifnonnull { branch } => {
                let iter_b = insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. ifnonnull", pos), &""]);
                insert_with_values(store, Some(&iter_b), None, &[0, 1], &[&"branch", &format!("{} ({})", branch, branch as i32 + *pos as i32)]);
                *pos += 3;
            }
            ifnull { branch } => {
                let iter_b = insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. ifnull", pos), &""]);
                insert_with_values(store, Some(&iter_b), None, &[0, 1], &[&"branch", &format!("{} ({})", branch, branch as i32 + *pos as i32)]);
                *pos += 3;
            }
            iinc { index, const_ } => {
                let iter_b = insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. iinc", pos), &""]);
                insert_with_values(store, Some(&iter_b), None, &[0, 1], &[&"index", &format!("{}", index)]);
                insert_with_values(store, Some(&iter_b), None, &[0, 1], &[&"const", &format!("{}", const_)]);
                *pos += 3;
            }
            iload { index } => {
                let iter_b = insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. iload", pos), &""]);
                insert_with_values(store, Some(&iter_b), None, &[0, 1], &[&"index", &format!("{}", index)]);
                *pos += 2;
            }
            iload_0 => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. iload_0", pos), &""]);
                *pos += 1;
            }
            iload_1 => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. iload_1", pos), &""]);
                *pos += 1;
            }
            iload_2 => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. iload_2", pos), &""]);
                *pos += 1;
            }
            iload_3 => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. iload_3", pos), &""]);
                *pos += 1;
            }
            imul => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. imul", pos), &""]);
                *pos += 1;
            }
            ineg => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. ineg", pos), &""]);
                *pos += 1;
            }
            instanceof { index } => {
                let iter_b = insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. instanceof", pos), &get_name(cp, index)]);
                insert_with_values(store, Some(&iter_b), None, &[0, 1], &[&"index", &format!("{}", index)]);
                *pos += 3;
            }
            invokedynamic { index } => {
                let iter_b = insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. invokedynamic", pos), &get_name(cp, index)]);
                insert_with_values(store, Some(&iter_b), None, &[0, 1], &[&"index", &format!("{}", index)]);
                *pos += 3;
            }
            invokeinterface { index, count } => {
                let iter_b = insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. invokeinterface", pos), &get_name(cp, index)]);
                insert_with_values(store, Some(&iter_b), None, &[0, 1], &[&"index", &format!("{}", index)]);
                insert_with_values(store, Some(&iter_b), None, &[0, 1], &[&"count", &format!("{}", count)]);
                *pos += 4;
            }
            invokespecial { index } => {
                let iter_b = insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. invokespecial", pos), &get_name(cp, index)]);
                insert_with_values(store, Some(&iter_b), None, &[0, 1], &[&"index", &format!("{}", index)]);
                *pos += 3;
            }
            invokestatic { index } => {
                let iter_b = insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. invokestatic", pos), &get_name(cp, index)]);
                insert_with_values(store, Some(&iter_b), None, &[0, 1], &[&"index", &format!("{}", index)]);
                *pos += 3;
            }
            invokevirtual { index } => {
                let iter_b = insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. invokevirtual", pos), &get_name(cp, index)]);
                insert_with_values(store, Some(&iter_b), None, &[0, 1], &[&"index", &format!("{}", index)]);
                *pos += 3;
            }
            ior => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. ior", pos), &""]);
                *pos += 1;
            }
            irem => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. irem", pos), &""]);
                *pos += 1;
            }
            ireturn => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. ireturn", pos), &""]);
                *pos += 1;
            }
            ishl => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. ishl", pos), &""]);
                *pos += 1;
            }
            ishr => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. ishr", pos), &""]);
                *pos += 1;
            }
            istore { index } => {
                let iter_b = insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. istore", pos), &""]);
                insert_with_values(store, Some(&iter_b), None, &[0, 1], &[&"index", &format!("{}", index)]);
                *pos += 2;
            }
            istore_0 => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. istore_0", pos), &""]);
                *pos += 1;
            }
            istore_1 => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. istore_1", pos), &""]);
                *pos += 1;
            }
            istore_2 => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. istore_2", pos), &""]);
                *pos += 1;
            }
            istore_3 => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. istore_3", pos), &""]);
                *pos += 1;
            }
            isub => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. isub", pos), &""]);
                *pos += 1;
            }
            iushr => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. iushr", pos), &""]);
                *pos += 1;
            }
            ixor => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. ixor", pos), &""]);
                *pos += 1;
            }
            jsr { branch } => {
                let iter_b = insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. jsr", pos), &""]);
                insert_with_values(store, Some(&iter_b), None, &[0, 1], &[&"branch", &format!("{} ({})", branch, branch as i32 + *pos as i32)]);
                *pos += 3;
            }
            jsr_w { branch } => {
                let iter_b = insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. jsr_w", pos), &""]);
                insert_with_values(store, Some(&iter_b), None, &[0, 1], &[&"branch", &format!("{} ({})", branch, branch as i32 + *pos as i32)]);
                *pos += 5;
            }
            l2d => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. l2d", pos), &""]);
                *pos += 1;
            }
            l2f => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. l2f", pos), &""]);
                *pos += 1;
            }
            l2i => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. l2i", pos), &""]);
                *pos += 1;
            }
            ladd => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. ladd", pos), &""]);
                *pos += 1;
            }
            laload => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. laload", pos), &""]);
                *pos += 1;
            }
            land => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. land", pos), &""]);
                *pos += 1;
            }
            lastore => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. lastore", pos), &""]);
                *pos += 1;
            }
            lcmp => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. lcmp", pos), &""]);
                *pos += 1;
            }
            lconst_0 => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. lconst_0", pos), &""]);
                *pos += 1;
            }
            lconst_1 => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. lconst_1", pos), &""]);
                *pos += 1;
            }
            ldc { index } => {
                let iter_b = insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. ldc", pos), &get_name(cp, index)]);
                insert_with_values(store, Some(&iter_b), None, &[0, 1], &[&"index", &format!("{}", index)]);
                *pos += 2;
            }
            ldc_w { index } => {
                let iter_b = insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. ldc_w", pos), &get_name(cp, index)]);
                insert_with_values(store, Some(&iter_b), None, &[0, 1], &[&"index", &format!("{}", index)]);
                *pos += 3;
            }
            ldc2_w { index } => {
                let iter_b = insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. ldc2_w", pos), &get_name(cp, index)]);
                insert_with_values(store, Some(&iter_b), None, &[0, 1], &[&"index", &format!("{}", index)]);
                *pos += 3;
            }
            ldiv => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. ldiv", pos), &""]);
                *pos += 1;
            }
            lload_0 => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. lload_0", pos), &""]);
                *pos += 1;
            }
            lload_1 => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. lload_1", pos), &""]);
                *pos += 1;
            }
            lload_2 => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. lload_2", pos), &""]);
                *pos += 1;
            }
            lload_3 => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. lload_3", pos), &""]);
                *pos += 1;
            }
            lload { index } => {
                let iter_b = insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. lload", pos), &""]);
                insert_with_values(store, Some(&iter_b), None, &[0, 1], &[&"index", &format!("{}", index)]);
                *pos += 2;
            }
            lmul => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. lmul", pos), &""]);
                *pos += 1;
            }
            lneg => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. lneg", pos), &""]);
                *pos += 1;
            }
            lookupswitch { default, match_offset_pairs, .. } => {
                let iter_b = insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. lookupswitch", pos), &""]);
                insert_with_values(store, Some(&iter_b), None, &[0, 1], &[&"default", &format!("{} ({})", default, default as u32 + *pos)]);
                let iter_c = insert_with_values(store, Some(&iter_b), None, &[0, 1], &[&"match_offset_pairs", &""]);
                for pair in &match_offset_pairs {
                    insert_with_values(store, Some(&iter_c), None, &[0, 1], &[&format!("{}", pair.0), &format!("{} ({})", pair.1, pair.1 as u32 + *pos)]);
                }
                *pos += 5 + (&match_offset_pairs.len() * 8) as u32;
            }
            lor => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. lor", pos), &""]);
                *pos += 1;
            }
            lrem => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. lrem", pos), &""]);
                *pos += 1;
            }
            lreturn => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. lreturn", pos), &""]);
                *pos += 1;
            }
            lshl => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. lshl", pos), &""]);
                *pos += 1;
            }
            lshr => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. lshr", pos), &""]);
                *pos += 1;
            }
            lstore { index } => {
                let iter_b = insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. lstore", pos), &""]);
                insert_with_values(store, Some(&iter_b), None, &[0, 1], &[&"index", &format!("{}", index)]);
                *pos += 2;
            }
            lstore_0 => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. lstore_0", pos), &""]);
                *pos += 1;
            }
            lstore_1 => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. lstore_1", pos), &""]);
                *pos += 1;
            }
            lstore_2 => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. lstore_2", pos), &""]);
                *pos += 1;
            }
            lstore_3 => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. lstore_3", pos), &""]);
                *pos += 1;
            }
            lsub => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. lsub", pos), &""]);
                *pos += 1;
            }
            lushr => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. lushr", pos), &""]);
                *pos += 1;
            }
            lxor => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. lxor", pos), &""]);
                *pos += 1;
            }
            monitorenter => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. monitorenter", pos), &""]);
                *pos += 1;
            }
            monitorexit => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. monitorexit", pos), &""]);
                *pos += 1;
            }
            multianewarray { index, dimensions } => {
                let iter_b = insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. multianewarray", pos), &get_name(cp, index)]);
                insert_with_values(store, Some(&iter_b), None, &[0, 1], &[&"index", &format!("{}", index)]);
                insert_with_values(store, Some(&iter_b), None, &[0, 1], &[&"dimensions", &format!("{}", dimensions)]);
                *pos += 4;
            }
            new { index } => {
                let iter_b = insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. new", pos), &get_name(cp, index)]);
                insert_with_values(store, Some(&iter_b), None, &[0, 1], &[&"index", &format!("{}", index)]);
                *pos += 3;
            }
            newarray { atype } => {
                let type_ = match atype {
                    4 => "T_BOOLEAN",
                    5 => "T_CHAR",
                    6 => "T_FLOAT",
                    7 => "T_DOUBLE",
                    8 => "T_BYTE",
                    9 => "T_SHORT",
                    10 => "T_INT",
                    11 => "T_LONG",
                    _ => "Invalid type code"
                };
                let iter_b = insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. newarray", pos), &type_]);
                insert_with_values(store, Some(&iter_b), None, &[0, 1], &[&"atype", &format!("{}", atype)]);
                *pos += 2;
            }
            nop => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. nop", pos), &""]);
                *pos += 1;
            }
            pop => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. pop", pos), &""]);
                *pos += 1;
            }
            pop2 => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. pop2", pos), &""]);
                *pos += 1;
            }
            putfield { index } => {
                let iter_b = insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. putfield", pos), &get_name(cp, index)]);
                insert_with_values(store, Some(&iter_b), None, &[0, 1], &[&"index", &format!("{}", index)]);
                *pos += 3;
            }
            putstatic { index } => {
                let iter_b = insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. putstatic", pos), &get_name(cp, index)]);
                insert_with_values(store, Some(&iter_b), None, &[0, 1], &[&"index", &format!("{}", index)]);
                *pos += 3;
            }
            ret { index } => {
                let iter_b = insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. ret", pos), &""]);
                insert_with_values(store, Some(&iter_b), None, &[0, 1], &[&"index", &format!("{}", index)]);
                *pos += 2;
            }
            return_ => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. return", pos), &""]);
                *pos += 1;
            }
            saload => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. saload", pos), &""]);
                *pos += 1;
            }
            sastore => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. sastore", pos), &""]);
                *pos += 1;
            }
            sipush { val } => {
                let iter_b = insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. sipush", pos), &""]);
                insert_with_values(store, Some(&iter_b), None, &[0, 1], &[&"val", &format!("{}", val)]);
                *pos += 3;
            }
            swap => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. swap", pos), &""]);
                *pos += 1;
            }
            tableswitch { default, low, high, jump_offsets, .. } => {
                let iter_b = insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. tableswitch", pos), &""]);
                insert_with_values(store, Some(&iter_b), None, &[0, 1], &[&"default", &format!("{} ({})", default, default as u32 + *pos)]);
                insert_with_values(store, Some(&iter_b), None, &[0, 1], &[&"low", &format!("{}", low)]);
                insert_with_values(store, Some(&iter_b), None, &[0, 1], &[&"high", &format!("{}", high)]);
                let iter_c = insert_with_values(store, Some(&iter_b), None, &[0, 1], &[&"match_offset_pairs", &""]);
                for offset in &jump_offsets {
                    insert_with_values(store, Some(&iter_c), None, &[0, 1], &[&format!("{} ({})", offset, *offset + *pos as i32), &""]);
                }
                *pos += 13 + (jump_offsets.len() * 8) as u32;
            }
            wide { opcode, index } => {
                let iter_b = insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. wide", pos), &""]);
                insert_with_values(store, Some(&iter_b), None, &[0, 1], &[&"opcode", &format!("{}", opcode)]);
                insert_with_values(store, Some(&iter_b), None, &[0, 1], &[&"index", &format!("{}", index)]);
                *pos += 4;
            }
            wide_iinc { index, const_ } => {
                let iter_b = insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. wide", pos), &""]);
                insert_with_values(store, Some(&iter_b), None, &[0, 1], &[&"opcode", &format!("{}", 0x84)]);
                insert_with_values(store, Some(&iter_b), None, &[0, 1], &[&"index", &format!("{}", index)]);
                insert_with_values(store, Some(&iter_b), None, &[0, 1], &[&"const", &format!("{}", const_)]);
                *pos += 6;
            }
            reserved => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. reserved", pos), &""]);
                *pos += 1;
            }
            impdep1 => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. impdep1", pos), &""]);
                *pos += 1;
            }
            impdep2 => {
                insert_with_values(store, Some(&iter_a), None, &[0, 1], &[&format!("{}. impdep2", pos), &""]);
                *pos += 1;
            }
        }
    }
}