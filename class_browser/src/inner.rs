use java_class::cp::ConstantPool;
use java_class::attributes::TypeAnnotation;
use java_class::attributes::Annotation;
use java_class::attributes::VerificationTypeInfo;
use java_class::attributes::StackMapFrame;
use java_class::opcodes::Opcode;
use java_class::opcodes::Opcode::*;
use java_class::attributes::Attribute::*;
use java_class::attributes::Attribute;
use java_class::cp_info::CPInfo;
use java_class::class::JavaClass;
use java_class::class::AccessFlags;
use gtk::prelude::*;
use gtk::*;
use std::str;
use java_class::attributes::ElementValue;
use java_class::attributes::TargetInfo;
use java_class::methods;
use java_class::fields;
use java_class::attributes;
use std::mem;

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
    let iter = ans.insert_with_values(None, None, &[0, 1], &[&"Class", &name]);
    ans.insert_with_values(Some(&iter), None, &[0, 1], &[&"minor_version", &format!("{}", class.minor_version)]);
    ans.insert_with_values(Some(&iter), None, &[0, 1], &[&"major_version", &format!("{}", class.major_version)]);
    let constants = &class.constant_pool;
    insert_constant_pool(&ans, &iter, constants);
    insert_access_class(&ans, &iter, class.access_flags);
    ans.insert_with_values(Some(&iter), None, &[0, 1], &[&"this_class", &format!("{}", class.this_class)]);
    ans.insert_with_values(Some(&iter), None, &[0, 1], &[&"super_class", &(format!("{} ({})", class.super_class, get_name(constants, class.super_class)))]);
    let interfaces = ans.insert_with_values(Some(&iter), None, &[0, 1], &[&"interfaces", &""]);
    for interface in class.interfaces {
        let name = get_name(&class.constant_pool, interface);
        ans.insert_with_values(Some(&interfaces), None, &[0, 1], &[&format!("{}", interface), &format!("{}", name)]);
    }
    let fields = ans.insert_with_values(Some(&iter), None, &[0, 1], &[&"Fields", &""]);
    for f in class.fields {
        let field = ans.insert_with_values(Some(&fields), None, &[0, 1], &[&"Field", &get_name(&class.constant_pool, f.name_index)]);
        insert_access_field(&ans, &field, f.access_flags);
        ans.insert_with_values(Some(&field), None, &[0, 1], &[&"name_index", &format!("{}", f.name_index)]);
        ans.insert_with_values(Some(&field), None, &[0, 1], &[&"descriptor_index", &format!("{}", f.descriptor_index)]);
        insert_attributes(constants, &ans, &field, f.attributes);
    }
    let methods = ans.insert_with_values(Some(&iter), None, &[0, 1], &[&"Methods", &""]);
    for m in class.methods {
        let method = ans.insert_with_values(Some(&methods), None, &[0, 1], &[&"Method", &get_name(&class.constant_pool, m.name_index)]);
        insert_access_method(&ans, &method, m.access_flags);
        ans.insert_with_values(Some(&method), None, &[0, 1], &[&"name_index", &format!("{}", m.name_index)]);
        ans.insert_with_values(Some(&method), None, &[0, 1], &[&"descriptor_index", &format!("{}", m.descriptor_index)]);
        insert_attributes(constants, &ans, &method, m.attributes);
    }
    insert_attributes(constants, &ans, &iter, class.attributes);
    tree.set_model(&ans);
    tree
}

fn get_name(cp: &ConstantPool, index: u16) -> String {
    match &cp[index] {
        CPInfo::Class { name_index } => { 
            get_name(cp, *name_index)
        },
        CPInfo::Utf8 {bytes, ..} => { str::from_utf8(bytes.as_slice()).unwrap().to_owned() },
        CPInfo::String { string_index } => { get_name(cp, *string_index) }
        CPInfo::NameAndType { name_index, descriptor_index } => {
            (get_name(cp, *name_index).to_owned()+" "+&get_name(cp, *descriptor_index))
        },
        CPInfo::Methodref { class_index, name_and_type_index } |
        CPInfo::Fieldref { class_index, name_and_type_index } |
        CPInfo::InterfaceMethodref { class_index, name_and_type_index } => {
            get_name(cp, *class_index).to_owned()+" "+&get_name(cp, *name_and_type_index)
        },
        CPInfo::Integer { bytes } => {
            format!("{}", *bytes as i32)
        },
        CPInfo::Float { bytes } => {
            unsafe {
                format!("{}", mem::transmute::<u32, f32>(*bytes))
            }
        },
        CPInfo::Long { bytes } => {
            format!("{}", *bytes as i64)
        },
        CPInfo::Double { bytes } => {
            unsafe {
                format!("{}", mem::transmute::<u64, f64>(*bytes))
            }
        },
        _ => "Constant Pool index did not point to Utf8".to_owned()
    }
}

fn insert_constant_pool(store: &TreeStore, iter: &TreeIter, constants: &ConstantPool) {
    let cp = store.insert_with_values(Some(&iter), None, &[0, 1], &[&"Constant Pool", &""]);
    for i in 1..=constants.items().len() as u16 {
        let cp_item = &constants[i];
        match cp_item {
            CPInfo::Class { name_index } => { 
                let iter_n = store.insert_with_values(Some(&cp), None, &[0, 1], &[&format!("{}. Class", i), &get_name(constants, *name_index)]);
                store.insert_with_values(Some(&iter_n), None, &[0, 1], &[&"name_index", &format!("{}", name_index)]);
             },
            CPInfo::Fieldref { class_index, name_and_type_index } => { 
                let iter_n = store.insert_with_values(Some(&cp), None, &[0, 1], &[&format!("{}. Fieldref", i), &get_name(constants, i)]);
                store.insert_with_values(Some(&iter_n), None, &[0, 1], &[&"class_index", &format!("{}", class_index)]);
                store.insert_with_values(Some(&iter_n), None, &[0, 1], &[&"name_and_type_index", &format!("{}", name_and_type_index)]);
            },
            CPInfo::Methodref { class_index, name_and_type_index } => { 
                let iter_n = store.insert_with_values(Some(&cp), None, &[0, 1], &[&format!("{}. Methodref", i), &get_name(constants, i)]);
                store.insert_with_values(Some(&iter_n), None, &[0, 1], &[&"class_index", &format!("{}", class_index)]);
                store.insert_with_values(Some(&iter_n), None, &[0, 1], &[&"name_and_type_index", &format!("{}", name_and_type_index)]);
            },
            CPInfo::InterfaceMethodref { class_index, name_and_type_index } => { 
                let iter_n = store.insert_with_values(Some(&cp), None, &[0, 1], &[&format!("{}. InterfaceMethodref", i), &get_name(constants, i)]);
                store.insert_with_values(Some(&iter_n), None, &[0, 1], &[&"class_index", &format!("{}", class_index)]);
                store.insert_with_values(Some(&iter_n), None, &[0, 1], &[&"name_and_type_index", &format!("{}", name_and_type_index)]);
            },
            CPInfo::String { string_index } => { 
                let iter_n = store.insert_with_values(Some(&cp), None, &[0, 1], &[&format!("{}. String", i), &get_name(constants, *string_index)]);
                store.insert_with_values(Some(&iter_n), None, &[0, 1], &[&"string_index", &format!("{}", string_index)]);
            },
            CPInfo::Integer { bytes } => { 
                let iter_n = store.insert_with_values(Some(&cp), None, &[0, 1], &[&format!("{}. Integer", i), &get_name(constants, i)]);
                store.insert_with_values(Some(&iter_n), None, &[0, 1], &[&"bytes", &format!("{}", bytes)]);
            },
            CPInfo::Float { bytes } => { 
                let iter_n = store.insert_with_values(Some(&cp), None, &[0, 1], &[&format!("{}. Float", i), &get_name(constants, i)]);
                store.insert_with_values(Some(&iter_n), None, &[0, 1], &[&"bytes", &format!("{}", bytes)]);
            },
            CPInfo::Long { bytes } => { 
                let iter_n = store.insert_with_values(Some(&cp), None, &[0, 1], &[&format!("{}. Long", i), &get_name(constants, i)]);
                store.insert_with_values(Some(&iter_n), None, &[0, 1], &[&"bytes", &format!("{}", bytes)]);
            },
            CPInfo::Double { bytes } => { 
                let iter_n = store.insert_with_values(Some(&cp), None, &[0, 1], &[&format!("{}. Double", i), &get_name(constants, i)]);
                store.insert_with_values(Some(&iter_n), None, &[0, 1], &[&"bytes", &format!("{}", bytes)]);
            },
            CPInfo::NameAndType { name_index, descriptor_index } => { 
                let iter_n = store.insert_with_values(Some(&cp), None, &[0, 1], &[&format!("{}. NameAndType", i), &(get_name(constants, i as u16))]);
                store.insert_with_values(Some(&iter_n), None, &[0, 1], &[&"name_index", &format!("{}", name_index)]);
                store.insert_with_values(Some(&iter_n), None, &[0, 1], &[&"descriptor_index", &format!("{}", descriptor_index)]);
            },
            CPInfo::Utf8 { length, bytes } => { 
                let iter_n = store.insert_with_values(Some(&cp), None, &[0, 1], &[&format!("{}. Utf8", i), &str::from_utf8(bytes).unwrap()]);
                store.insert_with_values(Some(&iter_n), None, &[0, 1], &[&"length", &format!("{}", length)]);
                let iter_bytes = store.insert_with_values(Some(&iter_n), None, &[0, 1], &[&"bytes", &str::from_utf8(bytes).unwrap()]);
                for byte in bytes {
                    store.insert_with_values(Some(&iter_bytes), None, &[0, 1], &[&format!("{}", byte), &""]);
                }
            },
            CPInfo::MethodHandle { reference_kind, reference_index } => { 
                let iter_n = store.insert_with_values(Some(&cp), None, &[0, 1], &[&format!("{}. MethodHandle", i), &get_name(constants, *reference_index)]);
                store.insert_with_values(Some(&iter_n), None, &[0, 1], &[&"reference_kind", &format!("{}", reference_kind)]);
                store.insert_with_values(Some(&iter_n), None, &[0, 1], &[&"reference_index", &format!("{}", reference_index)]);
            },
            CPInfo::MethodType { descriptor_index } => { 
                let iter_n = store.insert_with_values(Some(&cp), None, &[0, 1], &[&format!("{}. MethodType", i), &get_name(constants, *descriptor_index)]);
                store.insert_with_values(Some(&iter_n), None, &[0, 1], &[&"descriptor_index", &format!("{}", descriptor_index)]);
            },
            CPInfo::InvokeDynamic { bootstrap_method_attr_index, name_and_type_index } => { 
                let iter_n = store.insert_with_values(Some(&cp), None, &[0, 1], &[&format!("{}. InvokeDynamic", i), &get_name(constants, *name_and_type_index)]);
                store.insert_with_values(Some(&iter_n), None, &[0, 1], &[&"bootstrap_method_attr_index", &format!("{}", bootstrap_method_attr_index)]);
                store.insert_with_values(Some(&iter_n), None, &[0, 1], &[&"name_and_type_index", &format!("{}", name_and_type_index)]);
            },
        }
    }
}

fn insert_access_class(store: &TreeStore, iter: &TreeIter, access_flags: u16) {
    let access = store.insert_with_values(Some(&iter), None, &[0, 1], &[&"access_flags", &format!("{:#06X}", access_flags)]);
    store.insert_with_values(Some(&access), None, &[0, 1], &[&"public", &format!("{}", (access_flags & AccessFlags::Public as u16)!=0)]);
    store.insert_with_values(Some(&access), None, &[0, 1], &[&"final", &format!("{}", (access_flags & AccessFlags::Final as u16)!=0)]);
    store.insert_with_values(Some(&access), None, &[0, 1], &[&"super", &format!("{}", (access_flags & AccessFlags::Super as u16)!=0)]);
    store.insert_with_values(Some(&access), None, &[0, 1], &[&"interface", &format!("{}", (access_flags & AccessFlags::Interface as u16)!=0)]);
    store.insert_with_values(Some(&access), None, &[0, 1], &[&"abstract", &format!("{}", (access_flags & AccessFlags::Abstract as u16)!=0)]);
    store.insert_with_values(Some(&access), None, &[0, 1], &[&"synthetic", &format!("{}", (access_flags & AccessFlags::Synthetic as u16)!=0)]);
    store.insert_with_values(Some(&access), None, &[0, 1], &[&"annotation", &format!("{}", (access_flags & AccessFlags::Annotation as u16)!=0)]);
    store.insert_with_values(Some(&access), None, &[0, 1], &[&"enum", &format!("{}", (access_flags & AccessFlags::Enum as u16)!=0)]);
}

fn insert_access_field(store: &TreeStore, iter: &TreeIter, access_flags: u16) {
    let access = store.insert_with_values(Some(&iter), None, &[0, 1], &[&"access_flags", &format!("{:#06X}", access_flags)]);
    store.insert_with_values(Some(&access), None, &[0, 1], &[&"public", &format!("{}", (access_flags & fields::AccessFlags::Public as u16)!=0)]);
    store.insert_with_values(Some(&access), None, &[0, 1], &[&"private", &format!("{}", (access_flags & fields::AccessFlags::Private as u16)!=0)]);
    store.insert_with_values(Some(&access), None, &[0, 1], &[&"protected", &format!("{}", (access_flags & fields::AccessFlags::Protected as u16)!=0)]);
    store.insert_with_values(Some(&access), None, &[0, 1], &[&"static", &format!("{}", (access_flags & fields::AccessFlags::Static as u16)!=0)]);
    store.insert_with_values(Some(&access), None, &[0, 1], &[&"final", &format!("{}", (access_flags & fields::AccessFlags::Final as u16)!=0)]);
    store.insert_with_values(Some(&access), None, &[0, 1], &[&"volatile", &format!("{}", (access_flags & fields::AccessFlags::Volatile as u16)!=0)]);
    store.insert_with_values(Some(&access), None, &[0, 1], &[&"transient", &format!("{}", (access_flags & fields::AccessFlags::Transient as u16)!=0)]);
    store.insert_with_values(Some(&access), None, &[0, 1], &[&"synthetic", &format!("{}", (access_flags & fields::AccessFlags::Synthetic as u16)!=0)]);
    store.insert_with_values(Some(&access), None, &[0, 1], &[&"enum", &format!("{}", (access_flags & fields::AccessFlags::Enum as u16)!=0)]);
}

fn insert_access_method(store: &TreeStore, iter: &TreeIter, access_flags: u16) {
    let access = store.insert_with_values(Some(&iter), None, &[0, 1], &[&"access_flags", &format!("{:#06X}", access_flags)]);
    store.insert_with_values(Some(&access), None, &[0, 1], &[&"public", &format!("{}", (access_flags & methods::AccessFlags::Public as u16)!=0)]);
    store.insert_with_values(Some(&access), None, &[0, 1], &[&"private", &format!("{}", (access_flags & methods::AccessFlags::Private as u16)!=0)]);
    store.insert_with_values(Some(&access), None, &[0, 1], &[&"protected", &format!("{}", (access_flags & methods::AccessFlags::Protected as u16)!=0)]);
    store.insert_with_values(Some(&access), None, &[0, 1], &[&"static", &format!("{}", (access_flags & methods::AccessFlags::Static as u16)!=0)]);
    store.insert_with_values(Some(&access), None, &[0, 1], &[&"final", &format!("{}", (access_flags & methods::AccessFlags::Final as u16)!=0)]);
    store.insert_with_values(Some(&access), None, &[0, 1], &[&"synchronized", &format!("{}", (access_flags & methods::AccessFlags::Synchronized as u16)!=0)]);
    store.insert_with_values(Some(&access), None, &[0, 1], &[&"bridge", &format!("{}", (access_flags & methods::AccessFlags::Bridge as u16)!=0)]);
    store.insert_with_values(Some(&access), None, &[0, 1], &[&"varargs", &format!("{}", (access_flags & methods::AccessFlags::Varargs as u16)!=0)]);
    store.insert_with_values(Some(&access), None, &[0, 1], &[&"native", &format!("{}", (access_flags & methods::AccessFlags::Native as u16)!=0)]);
    store.insert_with_values(Some(&access), None, &[0, 1], &[&"abstract", &format!("{}", (access_flags & methods::AccessFlags::Abstract as u16)!=0)]);
    store.insert_with_values(Some(&access), None, &[0, 1], &[&"strict", &format!("{}", (access_flags & methods::AccessFlags::Strict as u16)!=0)]);
    store.insert_with_values(Some(&access), None, &[0, 1], &[&"synthetic", &format!("{}", (access_flags & methods::AccessFlags::Synthetic as u16)!=0)]);
}

fn insert_access_method_param(store: &TreeStore, iter: &TreeIter, access_flags: u16) {
    let access = store.insert_with_values(Some(&iter), None, &[0, 1], &[&"access_flags", &format!("{:#06X}", access_flags)]);
    store.insert_with_values(Some(&access), None, &[0, 1], &[&"final", &format!("{}", (access_flags & attributes::MethodParameterAccessFlags::Final as u16)!=0)]);
    store.insert_with_values(Some(&access), None, &[0, 1], &[&"synthetic", &format!("{}", (access_flags & attributes::MethodParameterAccessFlags::Synthetic as u16)!=0)]);
    store.insert_with_values(Some(&access), None, &[0, 1], &[&"mandated", &format!("{}", (access_flags & attributes::MethodParameterAccessFlags::Mandated as u16)!=0)]);
}

fn insert_attributes(cp: &ConstantPool, store: &TreeStore, iter: &TreeIter, attributes: Vec<Attribute>) {
    let iter_a = store.insert_with_values(Some(&iter), None, &[0, 1], &[&"Attributes", &""]);
    for attr in attributes {
        match attr {
            ConstantValue { constantvalue_index } => {
                let iter_b = store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"ConstantValue", &""]);
                store.insert_with_values(Some(&iter_b), None, &[0, 1], &[&"constantvalue_index", &format!("{}", constantvalue_index)]);
            },
            Code { max_stack, max_locals, code, exception_table, attributes } => {
                let iter_b = store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"Code", &""]);
                store.insert_with_values(Some(&iter_b), None, &[0, 1], &[&"max_stack", &format!("{}", max_stack)]);
                store.insert_with_values(Some(&iter_b), None, &[0, 1], &[&"max_locals", &format!("{}", max_locals)]);
                insert_code(store, &iter_b, code, cp);
                let iter_c = store.insert_with_values(Some(&iter_b), None, &[0, 1], &[&"exception_table", &""]);
                for i in 0..exception_table.len() {
                    let e = &exception_table[i];
                    let iter_d = store.insert_with_values(Some(&iter_c), None, &[0, 1], &[&format!("Entry {}", i), &""]);
                    store.insert_with_values(Some(&iter_d), None, &[0, 1], &[&"start_pc", &format!("{}", e.start_pc)]);
                    store.insert_with_values(Some(&iter_d), None, &[0, 1], &[&"end_pc", &format!("{}", e.end_pc)]);
                    store.insert_with_values(Some(&iter_d), None, &[0, 1], &[&"handler_pc", &format!("{}", e.handler_pc)]);
                    store.insert_with_values(Some(&iter_d), None, &[0, 1], &[&"catch_type", &format!("{}", e.catch_type)]);
                }
                insert_attributes(cp, store, &iter_c, attributes);
            },
            StackMapTable { entries } => {
                let iter_ba = store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"StackMapTable", &""]);
                let iter_b = store.insert_with_values(Some(&iter_ba), None, &[0, 1], &[&"Entries", &""]);
                for entry in entries {
                    match entry {
                        StackMapFrame::SameFrame => {
                            store.insert_with_values(Some(&iter_b), None, &[0, 1], &[&"SameFrame", &""]);
                        },
                        StackMapFrame::SameLocals1Item { stack } => {
                            let iter_c = store.insert_with_values(Some(&iter_b), None, &[0, 1], &[&"SameLocals1Item", &""]);
                            insert_vti(store, &iter_c, stack);
                        },
                        StackMapFrame::SameLocals1ItemExtended { offset_delta, stack } => {
                            let iter_c = store.insert_with_values(Some(&iter_b), None, &[0, 1], &[&"SameLocals1ItemExtended", &""]);
                            store.insert_with_values(Some(&iter_c), None, &[0, 1], &[&"offset_delta", &format!("{}", offset_delta)]);
                            insert_vti(store, &iter_c, stack);
                        },
                        StackMapFrame::ChopFrame { offset_delta } => {
                            let iter_c = store.insert_with_values(Some(&iter_b), None, &[0, 1], &[&"ChopFrame", &""]);
                            store.insert_with_values(Some(&iter_c), None, &[0, 1], &[&"offset_delta", &format!("{}", offset_delta)]);
                        },
                        StackMapFrame::SameFrameExtended { offset_delta } => {
                            let iter_c = store.insert_with_values(Some(&iter_b), None, &[0, 1], &[&"SameFrameExtended", &""]);
                            store.insert_with_values(Some(&iter_c), None, &[0, 1], &[&"offset_delta", &format!("{}", offset_delta)]);
                        },
                        StackMapFrame::AppendFrame { offset_delta, locals } => {
                            let iter_c = store.insert_with_values(Some(&iter_b), None, &[0, 1], &[&"SameLocals1ItemExtended", &""]);
                            store.insert_with_values(Some(&iter_c), None, &[0, 1], &[&"offset_delta", &format!("{}", offset_delta)]);
                            let iter_d = store.insert_with_values(Some(&iter_c), None, &[0, 1], &[&"locals", &""]);
                            for vti in locals {
                                insert_vti(store, &iter_d, vti);
                            }
                        },
                        StackMapFrame::FullFrame { offset_delta, locals, stack } => {
                            let iter_c = store.insert_with_values(Some(&iter_b), None, &[0, 1], &[&"FullFrame", &""]);
                            store.insert_with_values(Some(&iter_c), None, &[0, 1], &[&"offset_delta", &format!("{}", offset_delta)]);
                            let iter_d = store.insert_with_values(Some(&iter_c), None, &[0, 1], &[&"locals", &""]);
                            for vti in locals {
                                insert_vti(store, &iter_d, vti);
                            }
                            let iter_d = store.insert_with_values(Some(&iter_c), None, &[0, 1], &[&"stack", &""]);
                            for vti in stack {
                                insert_vti(store, &iter_d, vti);
                            }
                        },
                    }
                }
            },
            Exceptions { exception_index_table } => {
                let iter_b = store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"Exceptions", &""]);
                let iter_c = store.insert_with_values(Some(&iter_b), None, &[0, 1], &[&"exception_index_table", &""]);
                for i in exception_index_table {
                    store.insert_with_values(Some(&iter_c), None, &[0, 1], &[&format!("{}", i), &""]);
                }
            },
            InnerClasses { classes } => {
                let iter_b = store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"InnerClasses", &""]);
                for class in classes {
                    let iter_c = store.insert_with_values(Some(&iter_b), None, &[0, 1], &[&"InnerClass", &if class.inner_name_index==0 { "Anonymous class".to_owned() } else { get_name(cp, class.inner_name_index) }]);
                    store.insert_with_values(Some(&iter_c), None, &[0, 1], &[&"inner_java_class_index", &format!("{}", class.inner_class_info_index)]);
                    store.insert_with_values(Some(&iter_c), None, &[0, 1], &[&"outer_java_class_index", &format!("{}", class.outer_class_info_index)]);
                    store.insert_with_values(Some(&iter_c), None, &[0, 1], &[&"inner_name_index", &format!("{}", class.inner_name_index)]);
                    insert_access_class(store, &iter_c, class.inner_class_access_flags);
                }
            },
            EnclosingMethod { class_index, method_index } => {
                let iter_b = store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"ConstantValue", &""]);
                store.insert_with_values(Some(&iter_b), None, &[0, 1], &[&"class_index", &format!("{}", class_index)]);
                store.insert_with_values(Some(&iter_b), None, &[0, 1], &[&"method_index", &format!("{}", method_index)]);
            },
            Synthetic => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"Synthetic", &""]);
            },
            Signature { signature_index } => {
                let iter_b = store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"Signature", &""]);
                store.insert_with_values(Some(&iter_b), None, &[0, 1], &[&"signature_index", &format!("{}", signature_index)]);
            },
            SourceFile { sourcefile_index } => {
                let iter_b = store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"sourcefile_index", &""]);
                store.insert_with_values(Some(&iter_b), None, &[0, 1], &[&"sourcefile_index", &format!("{}", sourcefile_index)]);
            },
            SourceDebugExtenson { debug_extension } => {
                let iter_b = store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"SourceDebugExtension", &""]);
                let iter_c = store.insert_with_values(Some(&iter_b), None, &[0, 1], &[&"debug_extension", &""]);
                for i in debug_extension {
                    store.insert_with_values(Some(&iter_c), None, &[0, 1], &[&format!("{}", i), &""]);
                }
            },
            LineNumberTable { line_number_table } => {
                let iter_b = store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"LineNumberTable", &""]);
                for t in line_number_table {
                    let iter_c = store.insert_with_values(Some(&iter_b), None, &[0, 1], &[&"Entry", &""]);
                    store.insert_with_values(Some(&iter_c), None, &[0, 1], &[&"start_pc", &format!("{}", t.start_pc)]);
                    store.insert_with_values(Some(&iter_c), None, &[0, 1], &[&"line_nubmer", &format!("{}", t.line_number)]);
                }
            },
            LocalVariableTable { local_variable_table } => {
                let iter_b = store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"LocalVariableTable", &""]);
                for lv in local_variable_table {
                    let iter_c = store.insert_with_values(Some(&iter_b), None, &[0, 1], &[&"Entry", &""]);
                    store.insert_with_values(Some(&iter_c), None, &[0, 1], &[&"start_pc", &format!("{}", lv.start_pc)]);
                    store.insert_with_values(Some(&iter_c), None, &[0, 1], &[&"length", &format!("{}", lv.length)]);
                    store.insert_with_values(Some(&iter_c), None, &[0, 1], &[&"name_index", &format!("{}", lv.name_index)]);
                    store.insert_with_values(Some(&iter_c), None, &[0, 1], &[&"descriptor_index", &format!("{}", lv.descriptor_index)]);
                    store.insert_with_values(Some(&iter_c), None, &[0, 1], &[&"index", &format!("{}", lv.index)]);
                }
            },
            LocalVariableTypeTable { local_variable_type_table } => {
                let iter_b = store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"ConstantValue", &""]);
                for lv in local_variable_type_table {
                    let iter_c = store.insert_with_values(Some(&iter_b), None, &[0, 1], &[&"Entry", &""]);
                    store.insert_with_values(Some(&iter_c), None, &[0, 1], &[&"start_pc", &format!("{}", lv.start_pc)]);
                    store.insert_with_values(Some(&iter_c), None, &[0, 1], &[&"length", &format!("{}", lv.length)]);
                    store.insert_with_values(Some(&iter_c), None, &[0, 1], &[&"name_index", &format!("{}", lv.name_index)]);
                    store.insert_with_values(Some(&iter_c), None, &[0, 1], &[&"signature_index", &format!("{}", lv.signature_index)]);
                    store.insert_with_values(Some(&iter_c), None, &[0, 1], &[&"index", &format!("{}", lv.index)]);
                }
            },
            Deprecated => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"Deprecated", &""]);
            },
            RuntimeVisibleAnnotations { annotations } => {
                let iter_b = store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"RuntimeVisibleAnnotations", &""]);
                insert_annotations(store, &iter_b, annotations);
            },
            RuntimeInvisibleAnnotations { annotations } => {
                let iter_b = store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"RuntimeInvisibleAnnotations", &""]);
                insert_annotations(store, &iter_b, annotations);
            },
            RuntimeVisibleParameterAnnotations { parameter_annotations } => {
                let iter_b = store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"RuntimeVisibleParameterAnnotations", &""]);
                for i in 0..parameter_annotations.len() {
                    let iter_c = store.insert_with_values(Some(&iter_b), None, &[0, 1], &[&format!("{}", i), &""]);
                    for a in &parameter_annotations[i] {
                        insert_annotation(store, &iter_c, a);
                    }
                }
            },
            RuntimeInvisibleParameterAnnotations { parameter_annotations } => {
                let iter_b = store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"RuntimeInvisibleParameterAnnotations", &""]);
                for i in 0..parameter_annotations.len() {
                    let iter_c = store.insert_with_values(Some(&iter_b), None, &[0, 1], &[&format!("{}", i), &""]);
                    for a in &parameter_annotations[i] {
                        insert_annotation(store, &iter_c, a);
                    }
                }
            },
            RuntimeVisibleTypeAnnotations { annotations } => {
                let iter_b = store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"RuntimeVisibleTypeAnnotations", &""]);
                insert_type_annotations(store, &iter_b, annotations);
            },
            RuntimeInvisibleTypeAnnotations { annotations } => {
                let iter_b = store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"RuntimeInvisibleTypeAnnotations", &""]);
                insert_type_annotations(store, &iter_b, annotations);
            },
            AnnotationDefault { default_value } => {
                let iter_b = store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"AnnotationDefault", &""]);
                let iter_c = store.insert_with_values(Some(&iter_b), None, &[0, 1], &[&"default_value", &""]);
                insert_element_value(store, &iter_c, &default_value);
            },
            BootstrapMethods { bootstrap_methods } => {
                let iter_b = store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"BootstrapMethods", &""]);
                for m in bootstrap_methods {
                    let iter_c = store.insert_with_values(Some(&iter_b), None, &[0, 1], &[&"Entry", &""]);
                    store.insert_with_values(Some(&iter_c), None, &[0, 1], &[&"bootstrap_method_ref", &format!("{}", m.bootstrap_method_ref)]);
                    let iter_d = store.insert_with_values(Some(&iter_c), None, &[0, 1], &[&"bootstrap_arguments", &""]);
                    for i in 0..m.bootstrap_arguments.len() {
                        store.insert_with_values(Some(&iter_d), None, &[0, 1], &[&format!("{}", i), &format!("{}", m.bootstrap_arguments[i])]);
                    }
                }
            },
            MethodParameters { parameters } => {
                let iter_b = store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"MethodParameters", &""]);
                for p in parameters {
                    let iter_c = store.insert_with_values(Some(&iter_b), None, &[0, 1], &[&"Entry", &get_name(cp, p.name_index)]);
                    store.insert_with_values(Some(&iter_c), None, &[0, 1], &[&"name_index", &format!("{}", p.name_index)]);
                    insert_access_method_param(store, &iter_c, p.access_flags);
                }
            }
        }
    }
}

fn insert_type_annotations(store: &TreeStore, iter: &TreeIter, annotations: Vec<TypeAnnotation>) {
    let iter_a = store.insert_with_values(Some(iter), None, &[0, 1], &[&"Annotations", &""]);
    for a in annotations {
        let iter_b = store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"TypeAnnotation", &""]);
        match a.target_info {
            TargetInfo::TypeParameterTarget { type_parameter_index } => {
                let iter_c = store.insert_with_values(Some(&iter_b), None, &[0, 1], &[&"TypeParameterTarget", &""]);
                store.insert_with_values(Some(&iter_c), None, &[0, 1], &[&"type_parameter_index", &format!("{}", type_parameter_index)]);
            },
            TargetInfo::SupertypeTarget { supertype_index } => {
                let iter_c = store.insert_with_values(Some(&iter_b), None, &[0, 1], &[&"SupertypeTarget", &""]);
                store.insert_with_values(Some(&iter_c), None, &[0, 1], &[&"supertype_index", &format!("{}", supertype_index)]);
            },
            TargetInfo::TypeParameterBoundTarget { type_parameter_index, bound_index } => {
                let iter_c = store.insert_with_values(Some(&iter_b), None, &[0, 1], &[&"TypeParameterTarget", &""]);
                store.insert_with_values(Some(&iter_c), None, &[0, 1], &[&"type_parameter_index", &format!("{}", type_parameter_index)]);
                store.insert_with_values(Some(&iter_c), None, &[0, 1], &[&"bound_index", &format!("{}", bound_index)]);
            },
            TargetInfo::EmptyTarget => {
                store.insert_with_values(Some(&iter_b), None, &[0, 1], &[&"EmptyTarget", &""]);
            },
            TargetInfo::FormalParameterTarget { formal_parameter_index } => {
                let iter_c = store.insert_with_values(Some(&iter_b), None, &[0, 1], &[&"FormalParameterTarget", &""]);
                store.insert_with_values(Some(&iter_c), None, &[0, 1], &[&"formal_parameter_index", &format!("{}", formal_parameter_index)]);
            },
            TargetInfo::ThrowsTarget { throws_type_index } => {
                let iter_c = store.insert_with_values(Some(&iter_b), None, &[0, 1], &[&"ThrowsTarget", &""]);
                store.insert_with_values(Some(&iter_c), None, &[0, 1], &[&"throws_type_index", &format!("{}", throws_type_index)]);
            },
            TargetInfo::LocalVarTarget { table } => {
                let iter_c = store.insert_with_values(Some(&iter_b), None, &[0, 1], &[&"LocalVarTarget", &""]);
                let iter_d = store.insert_with_values(Some(&iter_c), None, &[0, 1], &[&"table", &""]);
                for te in table {
                    store.insert_with_values(Some(&iter_d), None, &[0, 1], &[&"start_pc", &format!("{}", te.start_pc)]);
                    store.insert_with_values(Some(&iter_d), None, &[0, 1], &[&"length", &format!("{}", te.length)]);
                    store.insert_with_values(Some(&iter_d), None, &[0, 1], &[&"index", &format!("{}", te.index)]);
                }
            },
            TargetInfo::CatchTarget { exception_table_index } => {
                let iter_c = store.insert_with_values(Some(&iter_b), None, &[0, 1], &[&"CatchTarget", &""]);
                store.insert_with_values(Some(&iter_c), None, &[0, 1], &[&"exception_table_index", &format!("{}", exception_table_index)]);
            },
            TargetInfo::OffsetTarget { offset } => {
                let iter_c = store.insert_with_values(Some(&iter_b), None, &[0, 1], &[&"OffsetTarget", &""]);
                store.insert_with_values(Some(&iter_c), None, &[0, 1], &[&"offset", &format!("{}", offset)]);
            },
            TargetInfo::TypeArgumentTarget { offset, type_argument_index } => {
                let iter_c = store.insert_with_values(Some(&iter_b), None, &[0, 1], &[&"TypeArgumentTarget", &""]);
                store.insert_with_values(Some(&iter_c), None, &[0, 1], &[&"offset", &format!("{}", offset)]);
                store.insert_with_values(Some(&iter_c), None, &[0, 1], &[&"type_argument_index", &format!("{}", type_argument_index)]);
            }
        }
    }
}

fn insert_annotations(store: &TreeStore, iter: &TreeIter, annotations: Vec<Annotation>) {
    let iter_a = store.insert_with_values(Some(iter), None, &[0, 1], &[&"Annotations", &""]);
    for a in annotations {
        insert_annotation(store, &iter_a, &a);
    }
}

fn insert_annotation(store: &TreeStore, iter: &TreeIter, annotation: &Annotation) {
    let iter_b = store.insert_with_values(Some(iter), None, &[0, 1], &[&"Annotation", &""]);
        store.insert_with_values(Some(&iter_b), None, &[0, 1], &[&"type_index", &format!("{}", annotation.type_index)]);
        let iter_c = store.insert_with_values(Some(&iter_b), None, &[0, 1], &[&"element_value_pairs", &""]);
        for pair in &annotation.element_value_pairs {
            let iter_d = store.insert_with_values(Some(&iter_c), None, &[0, 1], &[&"Pair", &""]);
            store.insert_with_values(Some(&iter_d), None, &[0, 1], &[&"element_name_index", &format!("{}", pair.element_name_index)]);
            insert_element_value(store, &iter_d, &pair.value);
        }
}

fn insert_element_value(store: &TreeStore, iter: &TreeIter, val: &ElementValue) {
    match val {
                ElementValue::ConstValueIndex(a) => {
                    store.insert_with_values(Some(iter), None, &[0, 1], &[&"ConstValueIndex", &format!("{}", a)]);
                },
                ElementValue::EnumConstValue { type_name_index, const_name_index } => {
                    let iter_a = store.insert_with_values(Some(iter), None, &[0, 1], &[&"EnumConstValue", &""]);
                    store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"type_name_index", &format!("{}", type_name_index)]);
                    store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"const_name_index", &format!("{}", const_name_index)]);
                },
                ElementValue::ClassInfoIndex(a) => {
                    store.insert_with_values(Some(iter), None, &[0, 1], &[&"ClassInfoIndex", &format!("{}", a)]);
                },
                ElementValue::AnnotationValue(a) => {
                    let iter_a = store.insert_with_values(Some(iter), None, &[0, 1], &[&"AnnotationValue", &""]);
                    insert_annotation(store, &iter_a, &a);
                },
                ElementValue::ArrayValue(vs) => {
                    let iter_a = store.insert_with_values(Some(iter), None, &[0, 1], &[&"ArrayValue", &""]);
                    for v in vs {
                        insert_element_value(store, &iter_a, v);
                    }
                }
            }
}

fn insert_vti(store: &TreeStore, iter: &TreeIter, vti: VerificationTypeInfo) {
    match vti {
        VerificationTypeInfo::Top => {
            store.insert_with_values(Some(&iter), None, &[0, 1], &[&"VerificationTypeInfo", &"Top"]);
        },
        VerificationTypeInfo::Integer => {
            store.insert_with_values(Some(&iter), None, &[0, 1], &[&"VerificationTypeInfo", &"Integer"]);
        },
        VerificationTypeInfo::Float => {
            store.insert_with_values(Some(&iter), None, &[0, 1], &[&"VerificationTypeInfo", &"Float"]);
        },
        VerificationTypeInfo::Null => {
            store.insert_with_values(Some(&iter), None, &[0, 1], &[&"VerificationTypeInfo", &"Null"]);
        },
        VerificationTypeInfo::UninitializedThis => {
            store.insert_with_values(Some(&iter), None, &[0, 1], &[&"VerificationTypeInfo", &"UninitializedThis"]);
        },
        VerificationTypeInfo::Object { cpool_index } => {
            let iter_a = store.insert_with_values(Some(&iter), None, &[0, 1], &[&"VerificationTypeInfo", &"Object"]);
            store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"cpool_index", &format!("{}", cpool_index)]);
        },
        VerificationTypeInfo::UninitializedVariable { offset } => {
            let iter_a = store.insert_with_values(Some(&iter), None, &[0, 1], &[&"VerificationTypeInfo", &"UninitializedVariable"]);
            store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"cpool_index", &format!("{}", offset)]);
        },
        VerificationTypeInfo::Long => {
            store.insert_with_values(Some(&iter), None, &[0, 1], &[&"VerificationTypeInfo", &"Long"]);
        },
        VerificationTypeInfo::Double => {
            store.insert_with_values(Some(&iter), None, &[0, 1], &[&"VerificationTypeInfo", &"Double"]);
        },
    }
}

fn insert_code(store: &TreeStore, iter: &TreeIter, code: Vec<Opcode>, cp: &ConstantPool) {
    let iter_a = store.insert_with_values(Some(&iter), None, &[0, 1], &[&"Code", &""]);
    for op in code {
        match op {
         aaload => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"aaload", &""]);
            },
            aastore => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"aastore", &""]);
            },
            aconst_null => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"aconst_null", &""]);
            },
            aload { index } => {
                let iter_b = store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"aload", &""]);
                store.insert_with_values(Some(&iter_b), None, &[0, 1], &[&"index", &format!("{}", index)]);
            },
            aload_0 => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"aload_0", &""]);
            },
            aload_1 => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"aload_1", &""]);
            },
            aload_2 => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"aload_2", &""]);
            },
            aload_3 => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"aload_3", &""]);
            },
            anewarray { index } => {
                let iter_b = store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"anewarray", &get_name(cp, index)]);
                store.insert_with_values(Some(&iter_b), None, &[0, 1], &[&"index", &format!("{}", index)]);
            },
            areturn => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"areturn", &""]);
            },
            arraylength => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"arraylength", &""]);
            },
            astore { index } => {
                let iter_b = store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"astore", &""]);
                store.insert_with_values(Some(&iter_b), None, &[0, 1], &[&"index", &format!("{}", index)]);
            },
            astore_0 => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"astore_0", &""]);
            },
            astore_1 => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"astore_1", &""]);
            },
            astore_2 => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"astore_2", &""]);
            },
            astore_3 => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"astore_3", &""]);
            },
            athrow => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"athrow", &""]);
            },
            baload => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"baload", &""]);
            },
            bastore => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"bastore", &""]);
            },
            bipush { val } => {
                let iter_b = store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"bipush", &""]);
                store.insert_with_values(Some(&iter_b), None, &[0, 1], &[&"val", &format!("{}", val)]);
            },
            breakpoint => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"breakpoint", &""]);
            },
            caload => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"caload", &""]);
            },
            castore => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"castore", &""]);
            },
            checkcast { index } => {
                let iter_b = store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"checkcast", &get_name(cp, index)]);
                store.insert_with_values(Some(&iter_b), None, &[0, 1], &[&"index", &format!("{}", index)]);
            },
            d2f => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"d2f", &""]);
            },
            d2i => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"d2i", &""]);
            },
            d2l => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"d2l", &""]);
            },
            dadd => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"dadd", &""]);
            },
            daload => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"daload", &""]);
            },
            dastore => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"dastore", &""]);
            },
            dcmpg => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"dcmpg", &""]);
            },
            dcmpl => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"dcmpl", &""]);
            },
            dconst_0 => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"dconst_0", &""]);
            },
            dconst_1 => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"dconst_1", &""]);
            },
            ddiv => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"ddiv", &""]);
            },
            dload { index } => {
                let iter_b = store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"dload", &""]);
                store.insert_with_values(Some(&iter_b), None, &[0, 1], &[&"index", &format!("{}", index)]);
            },
            dload_0 => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"dload_0", &""]);
            },
            dload_1 => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"dload_1", &""]);
            },
            dload_2 => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"dload_2", &""]);
            },
            dload_3 => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"dload_3", &""]);
            },
            dmul => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"dmul", &""]);
            },
            dneg => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"dneg", &""]);
            },
            drem => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"drem", &""]);
            },
            dreturn => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"dreturn", &""]);
            },
            dstore { index } => {
                let iter_b = store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"dstore", &""]);
                store.insert_with_values(Some(&iter_b), None, &[0, 1], &[&"index", &format!("{}", index)]);
            },
            dstore_0 => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"dstore_0", &""]);
            },
            dstore_1 => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"dstore_1", &""]);
            },
            dstore_2 => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"dstore_2", &""]);
            },
            dstore_3 => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"dstore_3", &""]);
            },
            dsub => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"dsub", &""]);
            },
            dup => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"dup", &""]);
            },
            dup_x1 => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"dup_x1", &""]);
            },
            dup_x2 => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"dup_x2", &""]);
            },
            dup2 => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"dup2", &""]);
            },
            dup2_x1 => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"dup2_x1", &""]);
            },
            dup2_x2 => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"dup2_x2", &""]);
            },
            f2d => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"f2d", &""]);
            },
            f2i => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"f2i", &""]);
            },
            f2l => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"f2l", &""]);
            },
            fadd => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"fadd", &""]);
            },
            faload => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"faload", &""]);
            },
            fastore => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"fastore", &""]);
            },
            fcmpg => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"fcmpg", &""]);
            },
            fcmpl => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"fcmpl", &""]);
            },
            fconst_0 => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"fconst_0", &""]);
            },
            fconst_1 => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"fconst_1", &""]);
            },
            fconst_2 => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"fconst_2", &""]);
            },
            fdiv => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"fdiv", &""]);
            },
            fload { index } => {
                let iter_b = store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"fload", &""]);
                store.insert_with_values(Some(&iter_b), None, &[0, 1], &[&"index", &format!("{}", index)]);
            },
            fload_0 => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"fload_0", &""]);
            },
            fload_1 => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"fload_1", &""]);
            },
            fload_2 => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"fload_2", &""]);
            },
            fload_3 => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"fload_3", &""]);
            },
            fmul => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"fmul", &""]);
            },
            fneg => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"fneg", &""]);
            },
            frem => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"frem", &""]);
            },
            freturn => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"freturn", &""]);
            },
            fstore { index } => {
                let iter_b = store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"fstore", &""]);
                store.insert_with_values(Some(&iter_b), None, &[0, 1], &[&"index", &format!("{}", index)]);
            },
            fstore_0 => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"fstore_0", &""]);
            },
            fstore_1 => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"fstore_1", &""]);
            },
            fstore_2 => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"fstore_2", &""]);
            },
            fstore_3 => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"fstore_3", &""]);
            },
            fsub => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"fsub", &""]);
            },
            getfield { index } => {
                let iter_b = store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"getfield", &get_name(cp, index)]);
                store.insert_with_values(Some(&iter_b), None, &[0, 1], &[&"index", &format!("{}", index)]);
            },
            getstatic { index } => {
                let iter_b = store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"getstatic", &get_name(cp, index)]);
                store.insert_with_values(Some(&iter_b), None, &[0, 1], &[&"index", &format!("{}", index)]);
            },
            goto { branch } => {
                let iter_b = store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"goto", &""]);
                store.insert_with_values(Some(&iter_b), None, &[0, 1], &[&"branch", &format!("{}", branch)]);
            },
            goto_w { branch } => {
                let iter_b = store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"goto_w", &""]);
                store.insert_with_values(Some(&iter_b), None, &[0, 1], &[&"branch", &format!("{}", branch)]);
            },
            i2b => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"i2b", &""]);
            },
            i2c => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"i2c", &""]);
            },
            i2d => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"i2d", &""]);
            },
            i2f => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"i2f", &""]);
            },
            i2l => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"i2l", &""]);
            },
            i2s => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"i2s", &""]);
            },
            iadd => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"iadd", &""]);
            },
            iaload => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"iaload", &""]);
            },
            iand => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"iand", &""]);
            },
            iastore => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"iastore", &""]);
            },
            iconst_m1 => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"iconst_m1", &""]);
            },
            iconst_0 => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"iconst_0", &""]);
            },
            iconst_1 => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"iconst_1", &""]);
            },
            iconst_2 => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"iconst_2", &""]);
            },
            iconst_3 => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"iconst_3", &""]);
            },
            iconst_4 => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"iconst_4", &""]);
            },
            iconst_5 => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"iconst_5", &""]);
            },
            idiv => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"idiv", &""]);
            },
            if_acmpeq { branch } => {
                let iter_b = store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"if_acmpeq", &""]);
                store.insert_with_values(Some(&iter_b), None, &[0, 1], &[&"branch", &format!("{}", branch)]);
            },
            if_acmpne { branch } => {
                let iter_b = store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"if_acmpne", &""]);
                store.insert_with_values(Some(&iter_b), None, &[0, 1], &[&"branch", &format!("{}", branch)]);
            },
            if_icmpeq { branch } => {
                let iter_b = store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"if_icmpeq", &""]);
                store.insert_with_values(Some(&iter_b), None, &[0, 1], &[&"branch", &format!("{}", branch)]);
            },
            if_icmpge { branch } => {
                let iter_b = store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"if_icmpge", &""]);
                store.insert_with_values(Some(&iter_b), None, &[0, 1], &[&"branch", &format!("{}", branch)]);
            },
            if_icmpgt { branch } => {
                let iter_b = store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"if_icmpgt", &""]);
                store.insert_with_values(Some(&iter_b), None, &[0, 1], &[&"branch", &format!("{}", branch)]);
            },
            if_icmple { branch } => {
                let iter_b = store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"if_icmple", &""]);
                store.insert_with_values(Some(&iter_b), None, &[0, 1], &[&"branch", &format!("{}", branch)]);
            },
            if_icmplt { branch } => {
                let iter_b = store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"if_icmplt", &""]);
                store.insert_with_values(Some(&iter_b), None, &[0, 1], &[&"branch", &format!("{}", branch)]);
            },
            if_icmpne { branch } => {
                let iter_b = store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"if_icmpne", &""]);
                store.insert_with_values(Some(&iter_b), None, &[0, 1], &[&"branch", &format!("{}", branch)]);
            },
            ifeq { branch } => {
                let iter_b = store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"ifeq", &""]);
                store.insert_with_values(Some(&iter_b), None, &[0, 1], &[&"branch", &format!("{}", branch)]);
            },
            ifge { branch } => {
                let iter_b = store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"ifge", &""]);
                store.insert_with_values(Some(&iter_b), None, &[0, 1], &[&"branch", &format!("{}", branch)]);
            },
            ifgt { branch } => {
                let iter_b = store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"ifgt", &""]);
                store.insert_with_values(Some(&iter_b), None, &[0, 1], &[&"branch", &format!("{}", branch)]);
            },
            ifle { branch } => {
                let iter_b = store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"ifle", &""]);
                store.insert_with_values(Some(&iter_b), None, &[0, 1], &[&"branch", &format!("{}", branch)]);
            },
            iflt { branch } => {
                let iter_b = store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"iflt", &""]);
                store.insert_with_values(Some(&iter_b), None, &[0, 1], &[&"branch", &format!("{}", branch)]);
            },
            ifne { branch } => {
                let iter_b = store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"ifne", &""]);
                store.insert_with_values(Some(&iter_b), None, &[0, 1], &[&"branch", &format!("{}", branch)]);
            },
            ifnonnull { branch } => {
                let iter_b = store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"ifnonnull", &""]);
                store.insert_with_values(Some(&iter_b), None, &[0, 1], &[&"branch", &format!("{}", branch)]);
            },
            ifnull { branch } => {
                let iter_b = store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"ifnull", &""]);
                store.insert_with_values(Some(&iter_b), None, &[0, 1], &[&"branch", &format!("{}", branch)]);
            },
            iinc { index, const_ } => {
                let iter_b = store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"iinc", &""]);
                store.insert_with_values(Some(&iter_b), None, &[0, 1], &[&"index", &format!("{}", index)]);
                store.insert_with_values(Some(&iter_b), None, &[0, 1], &[&"const", &format!("{}", const_)]);
            },
            iload { index } => {
                let iter_b = store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"iload", &""]);
                store.insert_with_values(Some(&iter_b), None, &[0, 1], &[&"index", &format!("{}", index)]);
            },
            iload_0 => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"iload_0", &""]);
            },
            iload_1 => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"iload_1", &""]);
            },
            iload_2 => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"iload_2", &""]);
            },
            iload_3 => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"iload_3", &""]);
            },
            imul => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"imul", &""]);
            },
            ineg => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"ineg", &""]);
            },
            instanceof { index } => {
                let iter_b = store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"instanceof", &get_name(cp, index)]);
                store.insert_with_values(Some(&iter_b), None, &[0, 1], &[&"index", &format!("{}", index)]);
            },
            invokedynamic { index } => {
                let iter_b = store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"invokedynamic", &get_name(cp, index)]);
                store.insert_with_values(Some(&iter_b), None, &[0, 1], &[&"index", &format!("{}", index)]);
            },
            invokeinterface { index, count } => {
                let iter_b = store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"invokeinterface", &get_name(cp, index)]);
                store.insert_with_values(Some(&iter_b), None, &[0, 1], &[&"index", &format!("{}", index)]);
                store.insert_with_values(Some(&iter_b), None, &[0, 1], &[&"count", &format!("{}", count)]);
            },
            invokespecial { index } => {
                let iter_b = store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"invokespecial", &get_name(cp, index)]);
                store.insert_with_values(Some(&iter_b), None, &[0, 1], &[&"index", &format!("{}", index)]);
            },
            invokestatic { index } => {
                let iter_b = store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"invokestatic", &get_name(cp, index)]);
                store.insert_with_values(Some(&iter_b), None, &[0, 1], &[&"index", &format!("{}", index)]);
            },
            invokevirtual { index } => {
                let iter_b = store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"invokevirtual", &get_name(cp, index)]);
                store.insert_with_values(Some(&iter_b), None, &[0, 1], &[&"index", &format!("{}", index)]);
            },
            ior => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"ior", &""]);
            },
            irem => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"irem", &""]);
            },
            ireturn => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"ireturn", &""]);
            },
            ishl => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"ishl", &""]);
            },
            ishr => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"ishr", &""]);
            },
            istore { index } => {
                let iter_b = store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"istore", &""]);
                store.insert_with_values(Some(&iter_b), None, &[0, 1], &[&"index", &format!("{}", index)]);
            },
            istore_0 => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"istore_0", &""]);
            },
            istore_1 => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"istore_1", &""]);
            },
            istore_2 => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"istore_2", &""]);
            },
            istore_3 => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"istore_3", &""]);
            },
            isub => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"isub", &""]);
            },
            iushr => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"iushr", &""]);
            },
            ixor => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"ixor", &""]);
            },
            jsr { branch } => {
                let iter_b = store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"jsr", &""]);
                store.insert_with_values(Some(&iter_b), None, &[0, 1], &[&"branch", &format!("{}", branch)]);
            },
            jsr_w { branch } => {
                let iter_b = store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"jsr_w", &""]);
                store.insert_with_values(Some(&iter_b), None, &[0, 1], &[&"branch", &format!("{}", branch)]);
            },
            l2d => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"l2d", &""]);
            },
            l2f => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"l2f", &""]);
            },
            l2i => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"l2i", &""]);
            },
            ladd => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"ladd", &""]);
            },
            laload => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"laload", &""]);
            },
            land => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"land", &""]);
            },
            lastore => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"lastore", &""]);
            },
            lcmp => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"lcmp", &""]);
            },
            lconst_0 => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"lconst_0", &""]);
            },
            lconst_1 => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"lconst_1", &""]);
            },
            ldc { index } => {
                let iter_b = store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"ldc", &get_name(cp, index.into())]);
                store.insert_with_values(Some(&iter_b), None, &[0, 1], &[&"index", &format!("{}", index)]);
            },
            ldc_w { index } => {
                let iter_b = store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"ldc_w", &get_name(cp, index)]);
                store.insert_with_values(Some(&iter_b), None, &[0, 1], &[&"index", &format!("{}", index)]);
            },
            ldc2_w { index } => {
                let iter_b = store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"ldc2_w", &get_name(cp, index)]);
                store.insert_with_values(Some(&iter_b), None, &[0, 1], &[&"index", &format!("{}", index)]);
            },
            ldiv => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"ldiv", &""]);
            },
            lload_0 => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"lload_0", &""]);
            },
            lload_1 => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"lload_1", &""]);
            },
            lload_2 => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"lload_2", &""]);
            },
            lload_3 => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"lload_3", &""]);
            },
            lload { index } => {
                let iter_b = store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"lload", &""]);
                store.insert_with_values(Some(&iter_b), None, &[0, 1], &[&"index", &format!("{}", index)]);
            },
            lmul => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"lmul", &""]);
            },
            lneg => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"lneg", &""]);
            },
            lookupswitch { default, match_offset_pairs } => {
                let iter_b = store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"lookupswitch", &""]);
                store.insert_with_values(Some(&iter_b), None, &[0, 1], &[&"default", &format!("{}", default)]);
                let iter_c = store.insert_with_values(Some(&iter_b), None, &[0, 1], &[&"match_offset_pairs", &""]);
                for pair in match_offset_pairs {
                    store.insert_with_values(Some(&iter_c), None, &[0, 1], &[&format!("{}", pair.0), &format!("{}", pair.1)]);
                }
            },
            lor => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"lor", &""]);
            },
            lrem => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"lrem", &""]);
            },
            lreturn => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"lreturn", &""]);
            },
            lshl => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"lshl", &""]);
            },
            lshr => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"lshr", &""]);
            },
            lstore { index } => {
                let iter_b = store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"lstore", &""]);
                store.insert_with_values(Some(&iter_b), None, &[0, 1], &[&"index", &format!("{}", index)]);
            },
            lstore_0 => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"lstore_0", &""]);
            },
            lstore_1 => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"lstore_1", &""]);
            },
            lstore_2 => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"lstore_2", &""]);
            },
            lstore_3 => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"lstore_3", &""]);
            },
            lsub => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"lsub", &""]);
            },
            lushr => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"lushr", &""]);
            },
            lxor => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"lxor", &""]);
            },
            monitorenter => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"monitorenter", &""]);
            },
            monitorexit => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"monitorexit", &""]);
            },
            multianewarray { index, dimensions } => {
                let iter_b = store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"multianewarray", &get_name(cp, index)]);
                store.insert_with_values(Some(&iter_b), None, &[0, 1], &[&"index", &format!("{}", index)]);
                store.insert_with_values(Some(&iter_b), None, &[0, 1], &[&"dimensions", &format!("{}", dimensions)]);
            },
            new { index } => {
                let iter_b = store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"new", &get_name(cp, index)]);
                store.insert_with_values(Some(&iter_b), None, &[0, 1], &[&"index", &format!("{}", index)]);
            },
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
                let iter_b = store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"newarray", &type_]);
                store.insert_with_values(Some(&iter_b), None, &[0, 1], &[&"atype", &format!("{}", atype)]);
            },
            nop => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"nop", &""]);
            },
            pop => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"pop", &""]);
            },
            pop2 => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"pop2", &""]);
            },
            putfield { index } => {
                let iter_b = store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"putfield", &get_name(cp, index)]);
                store.insert_with_values(Some(&iter_b), None, &[0, 1], &[&"index", &format!("{}", index)]);
            },
            putstatic { index } => {
                let iter_b = store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"putstatic", &get_name(cp, index)]);
                store.insert_with_values(Some(&iter_b), None, &[0, 1], &[&"index", &format!("{}", index)]);
            },
            ret { index } => {
                let iter_b = store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"ret", &""]);
                store.insert_with_values(Some(&iter_b), None, &[0, 1], &[&"index", &format!("{}", index)]);
            },
            return_ => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"return_", &""]);
            },
            saload => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"saload", &""]);
            },
            sastore => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"sastore", &""]);
            },
            sipush { val } => {
                let iter_b = store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"sipush", &""]);
                store.insert_with_values(Some(&iter_b), None, &[0, 1], &[&"val", &format!("{}", val)]);
            },
            swap => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"swap", &""]);
            },
            tableswitch { default, low, high, jump_offsets } => {
                let iter_b = store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"tableswitch", &""]);
                store.insert_with_values(Some(&iter_b), None, &[0, 1], &[&"default", &format!("{}", default)]);
                store.insert_with_values(Some(&iter_b), None, &[0, 1], &[&"low", &format!("{}", low)]);
                store.insert_with_values(Some(&iter_b), None, &[0, 1], &[&"high", &format!("{}", high)]);
                let iter_c = store.insert_with_values(Some(&iter_b), None, &[0, 1], &[&"match_offset_pairs", &""]);
                for pair in jump_offsets {
                    store.insert_with_values(Some(&iter_c), None, &[0, 1], &[&format!("{}", pair.0), &format!("{}", pair.1)]);
                }
            },
            wide { opcode, index, count } => {
                let iter_b = store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"wide", &""]);
                store.insert_with_values(Some(&iter_b), None, &[0, 1], &[&"opcode", &format!("{}", opcode)]);
                store.insert_with_values(Some(&iter_b), None, &[0, 1], &[&"index", &format!("{}", index)]);
                store.insert_with_values(Some(&iter_b), None, &[0, 1], &[&"count", &format!("{}", count)]);
            },
            reserved => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"reserved", &""]);
            },
            impdep1 => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"impdep1", &""]);
            },
            impdep2 => {
                store.insert_with_values(Some(&iter_a), None, &[0, 1], &[&"impdep2", &""]);
            }
        }
    }
}