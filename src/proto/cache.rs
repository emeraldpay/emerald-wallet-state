// This file is generated by rust-protobuf 2.25.2. Do not edit
// @generated

// https://github.com/rust-lang/rust-clippy/issues/702
#![allow(unknown_lints)]
#![allow(clippy::all)]

#![allow(unused_attributes)]
#![cfg_attr(rustfmt, rustfmt::skip)]

#![allow(box_pointers)]
#![allow(dead_code)]
#![allow(missing_docs)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(non_upper_case_globals)]
#![allow(trivial_casts)]
#![allow(unused_imports)]
#![allow(unused_results)]
//! Generated file from `cache.proto`

/// Generated files are compatible only with the same version
/// of protobuf runtime.
// const _PROTOBUF_VERSION_CHECK: () = ::protobuf::VERSION_2_25_2;

#[derive(PartialEq,Clone,Default)]
pub struct Cache {
    // message fields
    pub id: ::std::string::String,
    pub ts: u64,
    pub ttl: u64,
    pub value: ::std::string::String,
    // special fields
    pub unknown_fields: ::protobuf::UnknownFields,
    pub cached_size: ::protobuf::CachedSize,
}

impl<'a> ::std::default::Default for &'a Cache {
    fn default() -> &'a Cache {
        <Cache as ::protobuf::Message>::default_instance()
    }
}

impl Cache {
    pub fn new() -> Cache {
        ::std::default::Default::default()
    }

    // string id = 1;


    pub fn get_id(&self) -> &str {
        &self.id
    }
    pub fn clear_id(&mut self) {
        self.id.clear();
    }

    // Param is passed by value, moved
    pub fn set_id(&mut self, v: ::std::string::String) {
        self.id = v;
    }

    // Mutable pointer to the field.
    // If field is not initialized, it is initialized with default value first.
    pub fn mut_id(&mut self) -> &mut ::std::string::String {
        &mut self.id
    }

    // Take field
    pub fn take_id(&mut self) -> ::std::string::String {
        ::std::mem::replace(&mut self.id, ::std::string::String::new())
    }

    // uint64 ts = 2;


    pub fn get_ts(&self) -> u64 {
        self.ts
    }
    pub fn clear_ts(&mut self) {
        self.ts = 0;
    }

    // Param is passed by value, moved
    pub fn set_ts(&mut self, v: u64) {
        self.ts = v;
    }

    // uint64 ttl = 3;


    pub fn get_ttl(&self) -> u64 {
        self.ttl
    }
    pub fn clear_ttl(&mut self) {
        self.ttl = 0;
    }

    // Param is passed by value, moved
    pub fn set_ttl(&mut self, v: u64) {
        self.ttl = v;
    }

    // string value = 4;


    pub fn get_value(&self) -> &str {
        &self.value
    }
    pub fn clear_value(&mut self) {
        self.value.clear();
    }

    // Param is passed by value, moved
    pub fn set_value(&mut self, v: ::std::string::String) {
        self.value = v;
    }

    // Mutable pointer to the field.
    // If field is not initialized, it is initialized with default value first.
    pub fn mut_value(&mut self) -> &mut ::std::string::String {
        &mut self.value
    }

    // Take field
    pub fn take_value(&mut self) -> ::std::string::String {
        ::std::mem::replace(&mut self.value, ::std::string::String::new())
    }
}

impl ::protobuf::Message for Cache {
    fn is_initialized(&self) -> bool {
        true
    }

    fn merge_from(&mut self, is: &mut ::protobuf::CodedInputStream<'_>) -> ::protobuf::ProtobufResult<()> {
        while !is.eof()? {
            let (field_number, wire_type) = is.read_tag_unpack()?;
            match field_number {
                1 => {
                    ::protobuf::rt::read_singular_proto3_string_into(wire_type, is, &mut self.id)?;
                },
                2 => {
                    if wire_type != ::protobuf::wire_format::WireTypeVarint {
                        return ::std::result::Result::Err(::protobuf::rt::unexpected_wire_type(wire_type));
                    }
                    let tmp = is.read_uint64()?;
                    self.ts = tmp;
                },
                3 => {
                    if wire_type != ::protobuf::wire_format::WireTypeVarint {
                        return ::std::result::Result::Err(::protobuf::rt::unexpected_wire_type(wire_type));
                    }
                    let tmp = is.read_uint64()?;
                    self.ttl = tmp;
                },
                4 => {
                    ::protobuf::rt::read_singular_proto3_string_into(wire_type, is, &mut self.value)?;
                },
                _ => {
                    ::protobuf::rt::read_unknown_or_skip_group(field_number, wire_type, is, self.mut_unknown_fields())?;
                },
            };
        }
        ::std::result::Result::Ok(())
    }

    // Compute sizes of nested messages
    #[allow(unused_variables)]
    fn compute_size(&self) -> u32 {
        let mut my_size = 0;
        if !self.id.is_empty() {
            my_size += ::protobuf::rt::string_size(1, &self.id);
        }
        if self.ts != 0 {
            my_size += ::protobuf::rt::value_size(2, self.ts, ::protobuf::wire_format::WireTypeVarint);
        }
        if self.ttl != 0 {
            my_size += ::protobuf::rt::value_size(3, self.ttl, ::protobuf::wire_format::WireTypeVarint);
        }
        if !self.value.is_empty() {
            my_size += ::protobuf::rt::string_size(4, &self.value);
        }
        my_size += ::protobuf::rt::unknown_fields_size(self.get_unknown_fields());
        self.cached_size.set(my_size);
        my_size
    }

    fn write_to_with_cached_sizes(&self, os: &mut ::protobuf::CodedOutputStream<'_>) -> ::protobuf::ProtobufResult<()> {
        if !self.id.is_empty() {
            os.write_string(1, &self.id)?;
        }
        if self.ts != 0 {
            os.write_uint64(2, self.ts)?;
        }
        if self.ttl != 0 {
            os.write_uint64(3, self.ttl)?;
        }
        if !self.value.is_empty() {
            os.write_string(4, &self.value)?;
        }
        os.write_unknown_fields(self.get_unknown_fields())?;
        ::std::result::Result::Ok(())
    }

    fn get_cached_size(&self) -> u32 {
        self.cached_size.get()
    }

    fn get_unknown_fields(&self) -> &::protobuf::UnknownFields {
        &self.unknown_fields
    }

    fn mut_unknown_fields(&mut self) -> &mut ::protobuf::UnknownFields {
        &mut self.unknown_fields
    }

    fn as_any(&self) -> &dyn (::std::any::Any) {
        self as &dyn (::std::any::Any)
    }
    fn as_any_mut(&mut self) -> &mut dyn (::std::any::Any) {
        self as &mut dyn (::std::any::Any)
    }
    fn into_any(self: ::std::boxed::Box<Self>) -> ::std::boxed::Box<dyn (::std::any::Any)> {
        self
    }

    fn descriptor(&self) -> &'static ::protobuf::reflect::MessageDescriptor {
        Self::descriptor_static()
    }

    fn new() -> Cache {
        Cache::new()
    }

    fn descriptor_static() -> &'static ::protobuf::reflect::MessageDescriptor {
        static descriptor: ::protobuf::rt::LazyV2<::protobuf::reflect::MessageDescriptor> = ::protobuf::rt::LazyV2::INIT;
        descriptor.get(|| {
            let mut fields = ::std::vec::Vec::new();
            fields.push(::protobuf::reflect::accessor::make_simple_field_accessor::<_, ::protobuf::types::ProtobufTypeString>(
                "id",
                |m: &Cache| { &m.id },
                |m: &mut Cache| { &mut m.id },
            ));
            fields.push(::protobuf::reflect::accessor::make_simple_field_accessor::<_, ::protobuf::types::ProtobufTypeUint64>(
                "ts",
                |m: &Cache| { &m.ts },
                |m: &mut Cache| { &mut m.ts },
            ));
            fields.push(::protobuf::reflect::accessor::make_simple_field_accessor::<_, ::protobuf::types::ProtobufTypeUint64>(
                "ttl",
                |m: &Cache| { &m.ttl },
                |m: &mut Cache| { &mut m.ttl },
            ));
            fields.push(::protobuf::reflect::accessor::make_simple_field_accessor::<_, ::protobuf::types::ProtobufTypeString>(
                "value",
                |m: &Cache| { &m.value },
                |m: &mut Cache| { &mut m.value },
            ));
            ::protobuf::reflect::MessageDescriptor::new_pb_name::<Cache>(
                "Cache",
                fields,
                file_descriptor_proto()
            )
        })
    }

    fn default_instance() -> &'static Cache {
        static instance: ::protobuf::rt::LazyV2<Cache> = ::protobuf::rt::LazyV2::INIT;
        instance.get(Cache::new)
    }
}

impl ::protobuf::Clear for Cache {
    fn clear(&mut self) {
        self.id.clear();
        self.ts = 0;
        self.ttl = 0;
        self.value.clear();
        self.unknown_fields.clear();
    }
}

impl ::std::fmt::Debug for Cache {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        ::protobuf::text_format::fmt(self, f)
    }
}

impl ::protobuf::reflect::ProtobufValue for Cache {
    fn as_ref(&self) -> ::protobuf::reflect::ReflectValueRef {
        ::protobuf::reflect::ReflectValueRef::Message(self)
    }
}

static file_descriptor_proto_data: &'static [u8] = b"\
    \n\x0bcache.proto\x12\remerald.state\"O\n\x05Cache\x12\x0e\n\x02id\x18\
    \x01\x20\x01(\tR\x02id\x12\x0e\n\x02ts\x18\x02\x20\x01(\x04R\x02ts\x12\
    \x10\n\x03ttl\x18\x03\x20\x01(\x04R\x03ttl\x12\x14\n\x05value\x18\x04\
    \x20\x01(\tR\x05valueJ\x90\x02\n\x06\x12\x04\0\0\x08\x01\n\x08\n\x01\x0c\
    \x12\x03\0\0\x12\n\x08\n\x01\x02\x12\x03\x01\0\x16\n\n\n\x02\x04\0\x12\
    \x04\x03\0\x08\x01\n\n\n\x03\x04\0\x01\x12\x03\x03\x08\r\n\x0b\n\x04\x04\
    \0\x02\0\x12\x03\x04\x02\x10\n\x0c\n\x05\x04\0\x02\0\x05\x12\x03\x04\x02\
    \x08\n\x0c\n\x05\x04\0\x02\0\x01\x12\x03\x04\t\x0b\n\x0c\n\x05\x04\0\x02\
    \0\x03\x12\x03\x04\x0e\x0f\n\x0b\n\x04\x04\0\x02\x01\x12\x03\x05\x02\x10\
    \n\x0c\n\x05\x04\0\x02\x01\x05\x12\x03\x05\x02\x08\n\x0c\n\x05\x04\0\x02\
    \x01\x01\x12\x03\x05\t\x0b\n\x0c\n\x05\x04\0\x02\x01\x03\x12\x03\x05\x0e\
    \x0f\n\x0b\n\x04\x04\0\x02\x02\x12\x03\x06\x02\x11\n\x0c\n\x05\x04\0\x02\
    \x02\x05\x12\x03\x06\x02\x08\n\x0c\n\x05\x04\0\x02\x02\x01\x12\x03\x06\t\
    \x0c\n\x0c\n\x05\x04\0\x02\x02\x03\x12\x03\x06\x0f\x10\n\x0b\n\x04\x04\0\
    \x02\x03\x12\x03\x07\x02\x13\n\x0c\n\x05\x04\0\x02\x03\x05\x12\x03\x07\
    \x02\x08\n\x0c\n\x05\x04\0\x02\x03\x01\x12\x03\x07\t\x0e\n\x0c\n\x05\x04\
    \0\x02\x03\x03\x12\x03\x07\x11\x12b\x06proto3\
";

static file_descriptor_proto_lazy: ::protobuf::rt::LazyV2<::protobuf::descriptor::FileDescriptorProto> = ::protobuf::rt::LazyV2::INIT;

fn parse_descriptor_proto() -> ::protobuf::descriptor::FileDescriptorProto {
    ::protobuf::Message::parse_from_bytes(file_descriptor_proto_data).unwrap()
}

pub fn file_descriptor_proto() -> &'static ::protobuf::descriptor::FileDescriptorProto {
    file_descriptor_proto_lazy.get(|| {
        parse_descriptor_proto()
    })
}
