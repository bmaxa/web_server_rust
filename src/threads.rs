use super::server::*;

extern "C" {
    fn futex_acquire(m:*mut i32);
    fn futex_release(m:*mut i32);
}
pub struct Mutex {
    pub m_: i32
}
pub struct AutoLock<'a> {
    m_: &'a mut Mutex
}
impl Mutex{
    pub fn lock(&mut self){
        unsafe {futex_acquire(&mut self.m_);}
    }
    pub fn unlock(&mut self){
        unsafe { futex_release(&mut self.m_);}
    }
}
impl<'a> AutoLock<'a>{
    pub fn new(m:&mut Mutex)->AutoLock{
        let mut rc = AutoLock{m_:m};
        rc.m_.lock();
        rc
    }
    pub fn lock(&mut self){
        self.m_.lock();
    }
    pub fn unlock(&mut self){
        self.m_.unlock();
    }
}
impl<'a> Drop for AutoLock<'a>{
    fn drop(&mut self){
        self.m_.unlock();
    }
}

