#pragma once

#include <memory>
#include <seastar/core/future.hh>

namespace seastar {
namespace rust {

namespace internal {
namespace ffi {

// TODO: When it becomes possible to pass a raw waker to C++, we could
// keep the WaitState on the C++ side. The wait state could also be
// a continuation and we could avoid one allocation.

struct cpp_future_wrapper_vtable {
    // Checks whether the future is ready.
    // If it is ready, move-initializes the object at out_var and returns 1.
    // Otherwise - returns 0.
    int (*poll_fn)(void* fut, void* out_var);

    // Attaches a "wait state" object to the future. The wait state
    // is a Rust object used for synchronization with 
    void (*attach_wait_state)(void *fut, void* wait_state);

    // Destroys the future under the pointer.
    void (*destroy)(void* fut);
};

extern "C" void seastar_rs_wait_state_wake_and_detach(void* wait_state);

}
}

template<typename T>
class future_wrapper_base {
public:
    using future_type = seastar::future<T>;

private:
    future_type* _f;

public:
    future_wrapper_base(future_type&& f) {
        _f = new future_type(std::move(f));
    }
    ~future_wrapper_base() {
        delete _f;
    }

    future_wrapper_base(future_wrapper_base&& other) noexcept {
        _f = other._f;
        other._f = nullptr;
    }
    future_wrapper_base(const future_wrapper_base& other) = delete;

    future_wrapper_base& operator=(future_wrapper_base&& other) noexcept {
        delete _f;
        _f = other._f;
        other._f = nullptr;
    }
    future_wrapper_base& operator=(const future_wrapper_base& other) = delete;

    future_type unwrap() {
        assert(_f && "tried to unwrap an already-unwrapped future wrapper");
        future_type ret(std::move(*_f));
        delete _f;
        _f = nullptr;
        return ret;
    }

    // operator bool() const {
    //     return bool(_f();
    // }
};

// TODO: Assert correct alignment?
// TODO: Handle exceptions!
#define SEASTAR_RS_DEFINE_FUTURE_WRAPPER(type, name)                                    \
    const ::seastar::rust::internal::ffi::cpp_future_wrapper_vtable*                    \
    seastar_rs_cpp_future_wrapper_vtbl_##label() {                                      \
        static const ::seastar::rust::internal::ffi::cpp_future_wrapper_vtable vtbl = { \
            .poll_fn = [] (void* fut, void* out_var) -> int {                           \
                auto* typed_fut = reinterpret_cast<::seastar::future<type>*>(fut);      \
                if (typed_fut->available()) {                                           \
                    new(out_var) type(fut.get());                                       \
                    return 1;                                                           \
                }                                                                       \
                return 0;                                                               \
            },                                                                          \
            .attach_wait_state = [] (void* fut, void* wait_state) {                     \
                auto* typed_fut = reinterpret_cast<::seastar::future<type>*>(fut);      \
                fut->then([wait_state] {                                                \
                    seastar_rs_wait_state_wake_and_detach(wait_state);                  \
                });                                                                     \
            },                                                                          \
            .destroy = [] (void* fut) {                                                 \
                auto* typed_fut = reinterpret_cast<::seastar::future<type>*>(fut);      \
                delete typed_fut;                                                       \
            }                                                                           \
        };                                                                              \
        return &vtbl;                                                                   \
    }                                                                                   \
    struct name : public ::seastar::rust::future_wrapper_base<type> {                   \
    public:                                                                             \
        using future_wrapper_base::future_wrapper_base;                                 \
    };                                                                                  \

}
}


