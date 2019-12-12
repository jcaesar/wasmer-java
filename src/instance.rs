use crate::{
    exception::{runtime_error, unwrap_or_throw, Error},
    types::{jptr, Pointer},
};
use jni::{
    objects::{GlobalRef, JClass, JObject, JString},
    sys::{jbyteArray, jint, jintArray, jlong},
    JNIEnv,
};
use std::{panic, rc::Rc};
use wasmer_runtime::{imports, instantiate, Export, Value};
use wasmer_runtime_core as core;

pub struct Instance {
    #[allow(unused)]
    java_instance_object: GlobalRef,
    instance: Rc<core::Instance>,
}

impl Instance {
    fn new(java_instance_object: GlobalRef, module_bytes: Vec<u8>) -> Result<Self, Error> {
        let module_bytes = module_bytes.as_slice();
        let imports = imports! {};
        let instance = match instantiate(module_bytes, &imports) {
            Ok(instance) => Rc::new(instance),
            Err(e) => {
                return Err(runtime_error(format!(
                    "Failed to instantiate the module: {}",
                    e
                )))
            }
        };

        Ok(Self {
            java_instance_object,
            instance,
        })
    }

    fn call_exported_function(
        &self,
        export_name: String,
        arguments: Vec<Value>,
    ) -> Result<Vec<Value>, Error> {
        let function = match self.instance.dyn_func(&export_name) {
            Ok(function) => function,
            Err(_) => {
                return Err(runtime_error(format!(
                    "Exported function `{}` does not exist.",
                    export_name
                )))
            }
        };

        function
            .call(arguments.as_slice())
            .map_err(|e| runtime_error(format!("{}", e)))
    }
}

#[no_mangle]
pub extern "system" fn Java_org_wasmer_Instance_instantiate(
    env: JNIEnv,
    _class: JClass,
    this: JObject,
    module_bytes: jbyteArray,
) -> jptr {
    let output = panic::catch_unwind(|| {
        let module_bytes = env.convert_byte_array(module_bytes)?;
        let java_instance = env.new_global_ref(this)?;

        let instance = Instance::new(java_instance, module_bytes)?;

        Ok(Pointer::new(instance).into())
    });

    unwrap_or_throw(&env, output)
}

#[no_mangle]
pub extern "system" fn Java_org_wasmer_Instance_drop(
    _env: JNIEnv,
    _class: JClass,
    instance_pointer: jptr,
) {
    let _: Pointer<Instance> = instance_pointer.into();
}

#[no_mangle]
pub extern "system" fn Java_org_wasmer_Instance_dynCall(
    env: JNIEnv,
    _class: JClass,
    instance_pointer: jlong,
    export_name: JString,
    arguments_pointer: jintArray,
) -> jint {
    let output = panic::catch_unwind(|| {
        let instance: &Instance = Into::<Pointer<Instance>>::into(instance_pointer).borrow();
        let export_name: String = env
            .get_string(export_name)
            .expect("Could not get Java string.")
            .into();

        let arguments_length = env.get_array_length(arguments_pointer).unwrap();

        let mut arguments: Vec<i32> = vec![0; arguments_length as usize];
        env.get_int_array_region(arguments_pointer, 0, arguments.as_mut_slice())
            .unwrap();

        let results = instance.call_exported_function(
            export_name,
            arguments
                .iter()
                .map(|argument| Value::I32(*argument))
                .collect(),
        )?;

        if results.len() > 0 {
            Ok(match results[0] {
                Value::I32(result) => result as jint,
                _ => unreachable!(),
            })
        } else {
            Ok(jint::default())
        }
    });

    unwrap_or_throw(&env, output)
}