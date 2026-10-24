//! Context-owned byte-oriented collection helpers matching the C++ catalog.
use crate::{Context, Error, Registry, Value};

#[derive(Debug, Default)]
pub struct CollectionList<T> { pub items: Vec<T> }
pub type StrList = CollectionList<crate::ByteString>;
pub type IntList = CollectionList<i64>;
pub type FloatList = CollectionList<f64>;
#[derive(Debug, Default)]
pub struct StrMap { pub items: Vec<(crate::ByteString, crate::ByteString)> }

trait Element: Clone + PartialEq + 'static {
    const TYPE: &'static str;
    fn get(value: &Value) -> Result<Self, Error>;
    fn put(value: Self) -> Value;
}
impl Element for crate::ByteString {
    const TYPE: &'static str = "string";
    fn get(value: &Value) -> Result<Self, Error> { Ok(value.as_string()?.clone()) }
    fn put(value: Self) -> Value { Value::string(value) }
}
impl Element for i64 {
    const TYPE: &'static str = "int";
    fn get(value: &Value) -> Result<Self, Error> { value.as_int() }
    fn put(value: Self) -> Value { Value::Int(value) }
}
impl Element for f64 {
    const TYPE: &'static str = "float";
    fn get(value: &Value) -> Result<Self, Error> { value.as_float() }
    fn put(value: Self) -> Value { Value::Float(value) }
}

fn index(i: i64, size: usize, method: &str) -> Result<usize, Error> {
    if i < 0 || i as u64 >= size as u64 {
        Err(Error::host(format!("{}: index out of range: {}", method, i)))
    } else {
        Ok(i as usize)
    }
}

fn register_list<T: Element>(registry: &mut Registry, type_name: &'static str) -> Result<(), Error> {
    registry.method(type_name, "len", &[], "int", move |ctx, this, _| {
        Ok(Value::Int(ctx.get::<CollectionList<T>>(this)?.items.len() as i64))
    })?;
    registry.method(type_name, "at", &["int"], T::TYPE, move |ctx, this, args| {
        let list = ctx.get::<CollectionList<T>>(this)?;
        let i = index(args[0].as_int()?, list.items.len(), &format!("{}.at", type_name))?;
        Ok(T::put(list.items[i].clone()))
    })?;
    registry.method(type_name, "add", &[T::TYPE], type_name, move |ctx, this, args| {
        ctx.get_mut::<CollectionList<T>>(this)?.items.push(T::get(&args[0])?);
        Ok(Value::object(type_name, this))
    })?;
    registry.method(type_name, "set", &["int", T::TYPE], type_name, move |ctx, this, args| {
        let list = ctx.get_mut::<CollectionList<T>>(this)?;
        let i = index(args[0].as_int()?, list.items.len(), &format!("{}.set", type_name))?;
        list.items[i] = T::get(&args[1])?;
        Ok(Value::object(type_name, this))
    })?;
    registry.method(type_name, "contains", &[T::TYPE], "bool", move |ctx, this, args| {
        let needle = T::get(&args[0])?;
        Ok(Value::Bool(ctx.get::<CollectionList<T>>(this)?.items.iter().any(|v| v == &needle)))
    })?;
    registry.method(type_name, "index_of", &[T::TYPE], "int", move |ctx, this, args| {
        let needle = T::get(&args[0])?;
        let list = &ctx.get::<CollectionList<T>>(this)?.items;
        Ok(Value::Int(list.iter().position(|v| v == &needle).map(|i| i as i64).unwrap_or(-1)))
    })?;
    registry.method(type_name, "clear", &[], type_name, move |ctx, this, _| {
        ctx.get_mut::<CollectionList<T>>(this)?.items.clear();
        Ok(Value::object(type_name, this))
    })?;
    Ok(())
}

fn make_collection<T: 'static>(ctx: &mut Context, type_name: &'static str, value: CollectionList<T>) -> Result<Value, Error> {
    Ok(Value::object(type_name, ctx.insert_collection(type_name, value)?))
}

/// Register the list and ordered string-map catalog in a registry.
pub fn register_collections(registry: &mut Registry) -> Result<&mut Registry, Error> {
    register_list::<crate::ByteString>(registry, "StrList")?;
    register_list::<i64>(registry, "IntList")?;
    register_list::<f64>(registry, "FloatList")?;
    registry.method("StrList", "join", &["string"], "string", |ctx, this, args| {
        let list = &ctx.get::<StrList>(this)?.items;
        let sep = args[0].as_bytes()?;
        let mut out = Vec::new();
        for (i, item) in list.iter().enumerate() {
            if i != 0 { out.extend_from_slice(sep); }
            out.extend_from_slice(item.as_bytes());
        }
        Ok(Value::string(out))
    })?;
    registry.function("str_list", &[], "StrList", |ctx, _| make_collection(ctx, "StrList", CollectionList::<crate::ByteString> { items: Vec::new() }))?;
    registry.function("int_list", &[], "IntList", |ctx, _| make_collection(ctx, "IntList", CollectionList::<i64> { items: Vec::new() }))?;
    registry.function("float_list", &[], "FloatList", |ctx, _| make_collection(ctx, "FloatList", CollectionList::<f64> { items: Vec::new() }))?;
    registry.function("str_map", &[], "StrMap", |ctx, _| Ok(Value::object("StrMap", ctx.insert_collection("StrMap", StrMap::default())?)))?;
    registry.function("str_split", &["string", "string"], "StrList", |ctx, args| {
        let source = args[0].as_bytes()?;
        let separator = args[1].as_bytes()?;
        if separator.is_empty() { return Err(Error::host("str_split: separator must not be empty")); }
        let mut items = Vec::new();
        let mut at = 0;
        while let Some(i) = source[at..].windows(separator.len()).position(|w| w == separator) {
            let p = at + i;
            items.push(crate::ByteString::from(&source[at..p]));
            at = p + separator.len();
        }
        items.push(crate::ByteString::from(&source[at..]));
        make_collection(ctx, "StrList", CollectionList { items })
    })?;
    registry.method("StrMap", "len", &[], "int", |ctx, this, _| Ok(Value::Int(ctx.get::<StrMap>(this)?.items.len() as i64)))?;
    registry.method("StrMap", "has", &["string"], "bool", |ctx, this, args| {
        let key = args[0].as_string()?;
        Ok(Value::Bool(ctx.get::<StrMap>(this)?.items.iter().any(|(k, _)| k == key)))
    })?;
    registry.method("StrMap", "get", &["string"], "string", |ctx, this, args| {
        let key = args[0].as_string()?;
        ctx.get::<StrMap>(this)?.items.iter().find(|(k, _)| k == key).map(|(_, v)| Value::string(v.clone())).ok_or_else(|| Error::host("StrMap.get: missing key"))
    })?;
    registry.method("StrMap", "get_or", &["string", "string"], "string", |ctx, this, args| {
        let key = args[0].as_string()?;
        let fallback = args[1].as_string()?.clone();
        Ok(ctx.get::<StrMap>(this)?.items.iter().find(|(k, _)| k == key).map(|(_, v)| Value::string(v.clone())).unwrap_or_else(|| Value::string(fallback)))
    })?;
    registry.method("StrMap", "set", &["string", "string"], "StrMap", |ctx, this, args| {
        let key = args[0].as_string()?.clone();
        let value = args[1].as_string()?.clone();
        let map = ctx.get_mut::<StrMap>(this)?;
        if let Some((_, v)) = map.items.iter_mut().find(|(k, _)| k == &key) { *v = value; } else { map.items.push((key, value)); }
        Ok(Value::object("StrMap", this))
    })?;
    registry.method("StrMap", "key_at", &["int"], "string", |ctx, this, args| {
        let map = ctx.get::<StrMap>(this)?;
        let i = index(args[0].as_int()?, map.items.len(), "StrMap.key_at")?;
        Ok(Value::string(map.items[i].0.clone()))
    })?;
    registry.method("StrMap", "remove", &["string"], "bool", |ctx, this, args| {
        let key = args[0].as_string()?;
        let map = ctx.get_mut::<StrMap>(this)?;
        if let Some(i) = map.items.iter().position(|(k, _)| k == key) { map.items.remove(i); Ok(Value::Bool(true)) } else { Ok(Value::Bool(false)) }
    })?;
    registry.method("StrMap", "clear", &[], "StrMap", |ctx, this, _| { ctx.get_mut::<StrMap>(this)?.items.clear(); Ok(Value::object("StrMap", this)) })?;
    Ok(registry)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ByteString;

    #[test]
    fn registration_has_the_complete_catalog() {
        let mut registry = Registry::new();
        register_collections(&mut registry).unwrap();
        assert_eq!(registry.signatures().len(), 35);
    }

    #[test]
    fn collection_quota_counts_only_collection_objects() {
        let mut context = Context::new();
        for _ in 0..65_536 { context.insert_collection("IntList", IntList::default()).unwrap(); }
        context.insert("Helper", 1u8);
        let error = context.insert_collection("StrMap", StrMap::default()).unwrap_err();
        assert_eq!(error.code, "RUN_HOST");
        assert_eq!(error.message, "StrMap object limit exceeded (65536)");
        let mut other = Context::new();
        other.insert_collection("StrList", StrList::default()).unwrap();
    }

    #[test]
    fn split_preserves_empty_fields_and_bytes() {
        let mut context = Context::new();
        let args = [Value::string(b",a,,".as_slice()), Value::string(b",".as_slice())];
        let result = {
            let mut registry = Registry::new();
            register_collections(&mut registry).unwrap();
            let (_, _, function) = registry.functions.get("fn str_split(string, string) -> StrList").unwrap();
            function(&mut context, &args).unwrap()
        };
        let object = result.as_obj().unwrap();
        assert_eq!(context.get::<StrList>(object).unwrap().items, vec![ByteString::from(b""), ByteString::from(b"a"), ByteString::from(b""), ByteString::from(b"")]);
    }
}

