pub mod sheet_lookup {
    use std::{
        collections::HashMap,
        sync::{Arc, LazyLock, Mutex},
    };

    use ironworks::{
        excel::{Excel, Field},
        file::exh::{ColumnDefinition, ColumnKind},
    };

    use crate::{cgwiki::formatter::format_string, exd_schema};

    #[derive(Clone)]
    pub struct SheetDetail {
        pub col_def_list: Vec<(String, ColumnDefinition)>,
    }

    static SHEET_DETAIL_CACHE: LazyLock<Mutex<HashMap<String, Arc<SheetDetail>>>> =
        LazyLock::new(|| Mutex::new(HashMap::new()));

    static CLASS_JOB_CACHE: LazyLock<Mutex<HashMap<u32, String>>> =
        LazyLock::new(|| Mutex::new(HashMap::new()));

    pub fn get_sheet_detail(excel: &Excel, sheet_name: &str) -> Option<Arc<SheetDetail>> {
        {
            let cache = SHEET_DETAIL_CACHE.lock().unwrap();
            if !cache.is_empty() {
                if let Some(cols) = cache.get(sheet_name) {
                    return Some(Arc::clone(cols));
                }
            }
        }

        let sheet = excel.sheet(sheet_name.to_string()).ok()?;

        // This is the order of fields in the file
        let original_column_list = sheet.columns().ok()?;

        // Take just the offsets, and sort them - they will match the order of names in the YAML file
        let mut field_offset_list: Vec<u32> = original_column_list
            .iter()
            .map(|x| get_shift_offset(x))
            .collect();
        field_offset_list.sort();

        // Skip first field "#", keep rest - each field corresponds to the offset at the same index
        // in the field offset list
        let mut field_name_list: Vec<String> = exd_schema::field_names(sheet_name)
            .ok()??
            .into_iter()
            .skip(1)
            .collect();

        {
            let mut unmap_index = 0;
            while field_name_list.len() < field_offset_list.len() {
                field_name_list.push(format!("unmapped_{unmap_index}"));
                unmap_index += 1;
            }
        }

        // For each offset in the original offset list (file order, desired)
        // - Find that offset in the field offset list
        // - Then, take that offset and index into the field name list to get the field name
        // - Then, take the field names and collect into a list of fields, in original file order
        let sorted_columns: Vec<String> = original_column_list
            .iter()
            .map(|off| {
                let field_index = field_offset_list
                    .binary_search(&get_shift_offset(off))
                    .unwrap();
                field_name_list[field_index].clone()
            })
            .collect();

        // Zip the field list with the column list to have a list of column names with definitions, in file order
        // This can be used to fetch specific fields in a row using just the name
        let zipped: Vec<(String, ColumnDefinition)> = sorted_columns
            .into_iter()
            .zip(original_column_list.into_iter())
            .collect();

        let sd: Arc<SheetDetail> = Arc::new(SheetDetail {
            col_def_list: zipped,
        });
        {
            let mut cache = SHEET_DETAIL_CACHE.lock().unwrap();
            cache.insert(sheet_name.to_string(), Arc::clone(&sd));
        }
        Some(sd)
    }

    pub fn lookup_sheet(
        excel: &Excel,
        sheet_name: &str,
        row_index: u32,
        col_name: &String,
    ) -> Option<String> {
        let sheet_detail = get_sheet_detail(excel, sheet_name)?;
        let sheet = excel.sheet(sheet_name).ok()?;
        let row = sheet.into_iter().find(|p| p.row_id() == row_index)?;

        let (_, column_def) = sheet_detail
            .col_def_list
            .iter()
            .find(|(x, _)| x == col_name)?;

        if let Field::String(sestring) = row.field(column_def).ok()? {
            let res = format_string(excel, &sestring);
            return Some(res);
        } else {
            return None;
        }
    }

    pub fn get_class_job(excel: &Excel, job_index: u32) -> Option<String> {
        {
            let cache = CLASS_JOB_CACHE.lock().unwrap();
            if !cache.is_empty() {
                if let Some(cols) = cache.get(&job_index) {
                    return Some(cols.clone());
                } else {
                    return None;
                }
            }
        }
        let sd = get_sheet_detail(excel, "ClassJob")?;
        let sheet = excel.sheet("ClassJob").ok()?;
        let (_, column_def) = sd.col_def_list.iter().find(|(x, _)| x == "NameEnglish")?;
        let mut temp_map: HashMap<u32, String> = HashMap::new();
        for row in sheet.into_iter() {
            let id = row.row_id();
            if id == 0 {
                temp_map.insert(id, String::from("adventurer"));
            } else {
                let field = row.field(column_def).ok()?;
                if let Field::String(sestring) = field {
                    let res = format_string(excel, &sestring);
                    temp_map.insert(id, res.to_lowercase());
                }
            }
        }
        {
            let mut cache = CLASS_JOB_CACHE.lock().unwrap();
            cache.clone_from(&temp_map);
        }
        temp_map.get(&job_index).cloned()
    }

    pub fn get_sheet_col(excel: &Excel, sheet_name: &str, column_index: u32) -> Option<String> {
        let sd = get_sheet_detail(excel, sheet_name)?;
        let (name, _) = sd.col_def_list.get(column_index as usize)?;
        Some(name.clone())
    }

    fn get_shift_offset(x: &ColumnDefinition) -> u32 {
        let shift: u32 = match x.kind() {
            ColumnKind::PackedBool1 => 1,
            ColumnKind::PackedBool2 => 2,
            ColumnKind::PackedBool3 => 3,
            ColumnKind::PackedBool4 => 4,
            ColumnKind::PackedBool5 => 5,
            ColumnKind::PackedBool6 => 6,
            ColumnKind::PackedBool7 => 7,
            _ => 0,
        };
        ((x.offset() as u32) << 3) + shift
    }
}
