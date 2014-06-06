use std::c_vec::CVec;
use std::mem;
use libc::{c_long,size_t,c_void};
use collections::HashMap;
use super::ffi::{easy,err,info,opt};
use {Response,Headers,header};

pub struct Handle {
  curl: *easy::CURL
}

impl Handle {
  pub fn new() -> Handle {
    Handle {
      curl: unsafe { easy::curl_easy_init() }
    }
  }

  #[inline]
  pub fn setopt<T: opt::OptVal>(&mut self, option: opt::Opt, val: T) -> Result<(), err::ErrCode> {
    // TODO: Prevent setting callback related options
    let res = unsafe { easy::curl_easy_setopt(self.curl, option, val.to_c_repr()) };
    if res.is_success() { Ok(()) } else { Err(res) }
  }

  #[inline]
  pub fn perform(&mut self) -> Result<Response, err::ErrCode> {
    let mut builder = ResponseBuilder::new();

    unsafe {
      let p: uint = mem::transmute(&builder);

      // Set callback options
      easy::curl_easy_setopt(self.curl, opt::READFUNCTION, curl_read_fn);
      easy::curl_easy_setopt(self.curl, opt::READDATA, 0);

      easy::curl_easy_setopt(self.curl, opt::WRITEFUNCTION, curl_write_fn);
      easy::curl_easy_setopt(self.curl, opt::WRITEDATA, p);

      easy::curl_easy_setopt(self.curl, opt::HEADERFUNCTION, curl_header_fn);
      easy::curl_easy_setopt(self.curl, opt::HEADERDATA, p);
    }

    let err = unsafe { easy::curl_easy_perform(self.curl) };

    // If the request failed, abort here
    if !err.is_success() {
      return Err(err);
    }

    // Try to get the response code
    builder.code = try!(self.get_info_long(info::RESPONSE_CODE)) as uint;

    Ok(builder.build())
  }

  pub fn get_response_code(&self) -> Result<uint, err::ErrCode> {
    Ok(try!(self.get_info_long(info::RESPONSE_CODE)) as uint)
  }

  fn get_info_long(&self, key: info::Key) -> Result<c_long, err::ErrCode> {
    let v: c_long = 0;
    let res = unsafe { easy::curl_easy_getinfo(self.curl, key, &v) };

    if !res.is_success() {
      return Err(res);
    }

    Ok(v)
  }
}

impl Drop for Handle {
  fn drop(&mut self) {
    unsafe { easy::curl_easy_cleanup(self.curl) }
  }
}

struct ResponseBuilder {
  code: uint,
  hdrs: HashMap<String,Vec<String>>,
  body: Vec<u8>
}

impl ResponseBuilder {
  fn new() -> ResponseBuilder {
    ResponseBuilder {
      code: 0,
      hdrs: HashMap::new(),
      body: Vec::new()
    }
  }

  fn add_header(&mut self, name: &str, val: &str) {
    let name = name.to_string();

    let inserted = match self.hdrs.find_mut(&name) {
      Some(vals) => {
        vals.push(val.to_string());
        true
      }
      None => false
    };

    if !inserted {
      self.hdrs.insert(name, vec!(val.to_string()));
    }
  }

  fn build(self) -> Response {
    let ResponseBuilder { code, hdrs, body } = self;
    Response::new(code, hdrs, body)
  }
}

/*
 *
 * ===== Callbacks =====
 */

#[no_mangle]
pub extern "C" fn curl_read_fn(p: *u8, size: size_t, nmemb: size_t, user_data: *c_void) -> size_t {
  println!("READ {} {}", size, nmemb);
  0
}

#[no_mangle]
pub extern "C" fn curl_write_fn(p: *mut u8, size: size_t, nmemb: size_t, resp: *mut ResponseBuilder) -> size_t {
  if !resp.is_null() {
    let builder: &mut ResponseBuilder = unsafe { mem::transmute(resp) };
    let chunk = unsafe { CVec::new(p, (size * nmemb) as uint) };
    builder.body.push_all(chunk.as_slice());
  }

  size * nmemb
}

#[no_mangle]
pub extern "C" fn curl_header_fn(p: *mut u8, size: size_t, nmemb: size_t, resp: &mut ResponseBuilder) -> size_t {
  // TODO: Skip the first call (it seems to be the status line)

  let vec = unsafe { CVec::new(p, (size * nmemb) as uint) };

  match header::parse(vec.as_slice()) {
    Some((name, val)) => {
      resp.add_header(name, val);
    }
    None => {}
  }

  vec.len() as size_t
}
