use opcodes::Opcode;

/// enum containing JVM Attributes
/// for more information refer to the [JVM specification](https://docs.oracle.com/javase/specs/jvms/se8/html/index.html)
#[derive(Debug)]
pub enum Attribute {
    ConstantValue { 
        constantvalue_index: u16
    },
    Code {
        max_stack: u16,
        max_locals: u16,
        code: Vec<Opcode>,
        exception_table: Vec<ExceptionTableEntry>,
        attributes: Vec<Attribute>
    },
    StackMapTable {
        entries: Vec<StackMapFrame>
    },
    Exceptions {
        exception_index_table: Vec<u16>
    },
    InnerClasses {
        classes: Vec<InnerClassInfo>
    },
    EnclosingMethod {
        class_index: u16,
        method_index: u16
    },
    Synthetic,
    Signature {
        signature_index: u16
    },
    SourceFile {
        sourcefile_index: u16
    },
    SourceDebugExtenson {
        debug_extension: Vec<u8>
    },
    LineNumberTable {
        line_number_table: Vec<LineNumberTableEntry>
    },
    LocalVariableTable {
        local_variable_table: Vec<LocalVariableTableEntry>
    },
    LocalVariableTypeTable {
        local_variable_type_table: Vec<LocalVariableTypeTableEntry>
    },
    Deprecated,
    RuntimeVisibleAnnotations {
        annotations: Vec<Annotation>
    },
    RuntimeInvisibleAnnotations {
        annotations: Vec<Annotation>
    },
    RuntimeVisibleParameterAnnotations {
        parameter_annotations: Vec<Vec<Annotation>>
    },
    RuntimeInvisibleParameterAnnotations {
        parameter_annotations: Vec<Vec<Annotation>>
    },
    RuntimeVisibleTypeAnnotations {
        annotations: Vec<TypeAnnotation>
    },
    RuntimeInvisibleTypeAnnotations {
        annotations: Vec<TypeAnnotation>
    },
    AnnotationDefault {
        default_value: ElementValue
    },
    BootstrapMethods {
        bootstrap_methods: Vec<BootstrapMethodsEntry>
    },
    MethodParameters {
        parameters: Vec<MethodParameterEntry>
    }
}

pub enum MethodParameterAccessFlags {
    Final     = 0x0010,
    Synthetic = 0x1000,
    Mandated  = 0x8000
}

#[derive(Debug)]
pub struct MethodParameterEntry {
    pub name_index: u16,
    pub access_flags: u16
}

#[derive(Debug)]
pub struct BootstrapMethodsEntry {
    pub bootstrap_method_ref: u16,
    pub bootstrap_arguments: Vec<u16>
}

#[derive(Debug)]
pub struct TypeAnnotation {
    pub target_info: TargetInfo,
    pub target_path: TypePath,
    pub type_index: u16,
    pub element_value_pairs: Vec<ElementValuePair>
}

#[derive(Debug)]
pub enum TargetInfo {
    TypeParameterTarget { type_parameter_index: u8 },
    SupertypeTarget { supertype_index: u16 },
    TypeParameterBoundTarget { type_parameter_index: u8, bound_index: u8},
    EmptyTarget,
    FormalParameterTarget { formal_parameter_index: u8 },
    ThrowsTarget { throws_type_index: u16 },
    LocalVarTarget { table: Vec<LocalVarTagetTableEntry> },
    CatchTarget { exception_table_index: u16 },
    OffsetTarget { offset: u16 },
    TypeArgumentTarget { offset: u16, type_argument_index: u8 },
}

#[derive(Debug)]
pub struct TypePath {
    pub path: Vec<TypePathEntry>
}

#[derive(Debug)]
pub struct TypePathEntry {
    pub type_path_kind: u8,
    pub type_argument_index: u8
}

#[derive(Debug)]
pub struct LocalVarTagetTableEntry {
    pub start_pc: u16,
    pub length: u16,
    pub index: u16
}

#[derive(Debug)]
pub struct Annotation {
    pub type_index: u16,
    pub element_value_pairs: Vec<ElementValuePair>
}

#[derive(Debug)]
pub struct ElementValuePair {
    pub element_name_index: u16,
    pub value: ElementValue
}

#[derive(Debug)]
pub enum ElementValue {
    ConstValueIndex(u16),
    EnumConstValue { type_name_index: u16, const_name_index: u16 },
    ClassInfoIndex(u16),
    AnnotationValue(Annotation),
    ArrayValue(Vec<ElementValue>)
}

#[derive(Debug)]
pub struct LocalVariableTypeTableEntry {
    pub start_pc: u16,
    pub length: u16,
    pub name_index: u16,
    pub signature_index: u16,
    pub index: u16
}

#[derive(Debug)]
pub struct LocalVariableTableEntry {
    pub start_pc: u16,
    pub length: u16,
    pub name_index: u16,
    pub descriptor_index: u16,
    pub index: u16
}

#[derive(Debug)]
pub struct LineNumberTableEntry {
    pub start_pc: u16,
    pub line_number: u16
}

#[derive(Debug)]
pub struct InnerClassInfo {
    pub inner_class_info_index: u16,
    pub outer_class_info_index: u16,
    pub inner_name_index: u16,
    pub inner_class_access_flags: u16
}

#[derive(Debug)]
pub enum VerificationTypeInfo {
    Top, //0
    Integer, //1
    Float, //2
    Null, //5
    UninitializedThis, //6
    Object { cpool_index: u16 }, //7
    UninitializedVariable { offset: u16 } ,//8
    Long, //4
    Double //3
}

#[derive(Debug)]
pub enum StackMapFrame {
    SameFrame { offset_delta: u8 }, //0-63
    SameLocals1Item { offset_delta: u8, stack: VerificationTypeInfo }, // 64-127
    SameLocals1ItemExtended { offset_delta: u16, stack: VerificationTypeInfo }, //247
    ChopFrame { absent_locals: u8, offset_delta: u16 }, //248-250
    SameFrameExtended { offset_delta: u16 }, //251
    AppendFrame { offset_delta: u16, locals: Vec<VerificationTypeInfo> }, //252-254
    FullFrame { offset_delta: u16, locals: Vec<VerificationTypeInfo>, stack: Vec<VerificationTypeInfo> } //255
}

#[derive(Debug)]
pub struct ExceptionTableEntry {
    pub start_pc: u16,
    pub end_pc: u16,
    pub handler_pc: u16,
    pub catch_type: u16
}