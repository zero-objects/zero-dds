// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/core/xtypes/detail/Json.hpp — a small JSON tree: the form in which a DynamicData value crosses the C API.

#ifndef ZERODDS_DDS_CORE_XTYPES_DETAIL_JSON_HPP
#define ZERODDS_DDS_CORE_XTYPES_DETAIL_JSON_HPP


#include "zerodds/psm/cxx.hpp"
#include <cstdlib>
#include <string>
#include <utility>
#include <vector>

#include "dds/core/Exception.hpp"

namespace dds {
namespace core {
namespace xtypes {
namespace detail {

/// A JSON value. A number keeps its text, so a 32-bit integer or a double is written back as it
/// was read; the C API's dialect keeps 64-bit integers and `long double` in strings.
class Json {
public:
    /// The kinds of a value; `Json::Kind::Null` etc. name the enumerators.
    struct Kind {
        enum Value { Null, Bool, Number, String, Array, Object };
    };

    Json() : kind_(Kind::Null), bool_(false), text_(), items_(), fields_() {}
    static Json null() { return Json(); }
    static Json boolean(bool v) {
        Json j;
        j.kind_ = Kind::Bool;
        j.bool_ = v;
        return j;
    }
    /// A number from its text.
    static Json number(const std::string& text) {
        Json j;
        j.kind_ = Kind::Number;
        j.text_ = text;
        return j;
    }
    static Json string(const std::string& text) {
        Json j;
        j.kind_ = Kind::String;
        j.text_ = text;
        return j;
    }
    static Json array() {
        Json j;
        j.kind_ = Kind::Array;
        return j;
    }
    static Json object() {
        Json j;
        j.kind_ = Kind::Object;
        return j;
    }

    Kind::Value kind() const { return kind_; }
    bool as_bool() const { return bool_; }
    /// The text of a number or a string.
    const std::string& text() const { return text_; }
    std::vector<Json>& items() { return items_; }
    const std::vector<Json>& items() const { return items_; }

    /// The member `name` of an object, or null.
    const Json* find(const std::string& name) const {
        for (std::vector<std::pair<std::string, Json> >::const_iterator it = fields_.begin();
             it != fields_.end(); ++it) {
            if (it->first == name) {
                return &it->second;
            }
        }
        return 0;
    }
    Json* find(const std::string& name) {
        for (std::vector<std::pair<std::string, Json> >::iterator it = fields_.begin();
             it != fields_.end(); ++it) {
            if (it->first == name) {
                return &it->second;
            }
        }
        return 0;
    }
    /// Sets (adds or replaces) a member of an object.
    void set(const std::string& name, Json value) {
        if (Json* existing = find(name)) {
            *existing = ZERODDS_MOVE(value);
        } else {
            fields_.push_back(std::make_pair(name, value));
        }
    }
    /// Removes a member of an object; false when it was not there.
    bool erase(const std::string& name) {
        for (std::vector<std::pair<std::string, Json> >::iterator it = fields_.begin(); it != fields_.end(); ++it) {
            if (it->first == name) {
                fields_.erase(it);
                return true;
            }
        }
        return false;
    }
    const std::vector<std::pair<std::string, Json> >& fields() const { return fields_; }

    /// Equal as values: an object's members in any order.
    bool same(const Json& o) const {
        if (kind_ != o.kind_) {
            return false;
        }
        switch (kind_) {
        case Kind::Null:
            return true;
        case Kind::Bool:
            return bool_ == o.bool_;
        case Kind::Number:
        case Kind::String:
            return text_ == o.text_;
        case Kind::Array:
            if (items_.size() != o.items_.size()) {
                return false;
            }
            for (std::size_t i = 0; i < items_.size(); ++i) {
                if (!items_[i].same(o.items_[i])) {
                    return false;
                }
            }
            return true;
        case Kind::Object:
            if (fields_.size() != o.fields_.size()) {
                return false;
            }
            for (std::vector<std::pair<std::string, Json> >::const_iterator it = fields_.begin();
                 it != fields_.end(); ++it) {
                const Json* other = o.find(it->first);
                if (other == 0 || !it->second.same(*other)) {
                    return false;
                }
            }
            return true;
        }
        return false;
    }

    std::string dump() const {
        std::string out;
        write(out);
        return out;
    }

    static Json parse(const std::string& text) {
        Parser p(text);
        p.skip();
        Json value = p.value(0);
        p.skip();
        if (p.at != text.size()) {
            p.fail("trailing text");
        }
        return value;
    }

private:
    struct Parser {
        const std::string& s;
        std::size_t at;
        explicit Parser(const std::string& text) : s(text), at(0) {}
        void fail(const char* what) const {
            throw ::dds::core::InvalidDataError(std::string("JSON: ") + what);
        }
        void skip() {
            while (at < s.size() && (s[at] == ' ' || s[at] == '\n' || s[at] == '\t' || s[at] == '\r')) {
                ++at;
            }
        }
        bool eat(char c) {
            skip();
            if (at < s.size() && s[at] == c) {
                ++at;
                return true;
            }
            return false;
        }
        Json value(int depth) {
            if (depth > 128) {
                fail("nesting too deep");
            }
            skip();
            if (at >= s.size()) {
                fail("unexpected end");
            }
            const char c = s[at];
            if (c == '{') {
                ++at;
                Json j = object();
                if (eat('}')) {
                    return j;
                }
                do {
                    skip();
                    std::string name = str();
                    if (!eat(':')) {
                        fail("expected ':'");
                    }
                    j.set(name, value(depth + 1));
                } while (eat(','));
                if (!eat('}')) {
                    fail("expected '}'");
                }
                return j;
            }
            if (c == '[') {
                ++at;
                Json j = array();
                if (eat(']')) {
                    return j;
                }
                do {
                    j.items_.push_back(value(depth + 1));
                } while (eat(','));
                if (!eat(']')) {
                    fail("expected ']'");
                }
                return j;
            }
            if (c == '"') {
                return string(str());
            }
            if (s.compare(at, 4, "true") == 0) {
                at += 4;
                return boolean(true);
            }
            if (s.compare(at, 5, "false") == 0) {
                at += 5;
                return boolean(false);
            }
            if (s.compare(at, 4, "null") == 0) {
                at += 4;
                return null();
            }
            std::size_t start = at;
            while (at < s.size() && (s[at] == '-' || s[at] == '+' || s[at] == '.' || s[at] == 'e' ||
                                     s[at] == 'E' || (s[at] >= '0' && s[at] <= '9'))) {
                ++at;
            }
            if (start == at) {
                fail("unexpected character");
            }
            return number(s.substr(start, at - start));
        }
        std::string str() {
            if (at >= s.size() || s[at] != '"') {
                fail("expected a string");
            }
            ++at;
            std::string out;
            while (at < s.size() && s[at] != '"') {
                char c = s[at++];
                if (c != '\\') {
                    out.push_back(c);
                    continue;
                }
                if (at >= s.size()) {
                    fail("unterminated escape");
                }
                const char e = s[at++];
                switch (e) {
                case '"': out.push_back('"'); break;
                case '\\': out.push_back('\\'); break;
                case '/': out.push_back('/'); break;
                case 'b': out.push_back('\b'); break;
                case 'f': out.push_back('\f'); break;
                case 'n': out.push_back('\n'); break;
                case 'r': out.push_back('\r'); break;
                case 't': out.push_back('\t'); break;
                case 'u': {
                    if (at + 4 > s.size()) {
                        fail("short \\u escape");
                    }
                    unsigned code = static_cast<unsigned>(std::strtoul(s.substr(at, 4).c_str(), 0, 16));
                    at += 4;
                    // A surrogate pair is one code point.
                    if (code >= 0xD800 && code <= 0xDBFF && at + 6 <= s.size() && s[at] == '\\' &&
                        s[at + 1] == 'u') {
                        const unsigned low =
                            static_cast<unsigned>(std::strtoul(s.substr(at + 2, 4).c_str(), 0, 16));
                        if (low >= 0xDC00 && low <= 0xDFFF) {
                            code = 0x10000 + ((code - 0xD800) << 10) + (low - 0xDC00);
                            at += 6;
                        }
                    }
                    append_utf8(out, code);
                    break;
                }
                default: fail("bad escape");
                }
            }
            if (at >= s.size()) {
                fail("unterminated string");
            }
            ++at;
            return out;
        }
        static void append_utf8(std::string& out, unsigned code) {
            if (code < 0x80) {
                out.push_back(static_cast<char>(code));
            } else if (code < 0x800) {
                out.push_back(static_cast<char>(0xC0 | (code >> 6)));
                out.push_back(static_cast<char>(0x80 | (code & 0x3F)));
            } else if (code < 0x10000) {
                out.push_back(static_cast<char>(0xE0 | (code >> 12)));
                out.push_back(static_cast<char>(0x80 | ((code >> 6) & 0x3F)));
                out.push_back(static_cast<char>(0x80 | (code & 0x3F)));
            } else {
                out.push_back(static_cast<char>(0xF0 | (code >> 18)));
                out.push_back(static_cast<char>(0x80 | ((code >> 12) & 0x3F)));
                out.push_back(static_cast<char>(0x80 | ((code >> 6) & 0x3F)));
                out.push_back(static_cast<char>(0x80 | (code & 0x3F)));
            }
        }
    };

    static void quote(std::string& out, const std::string& text) {
        out.push_back('"');
        for (std::string::const_iterator ci = text.begin(); ci != text.end(); ++ci) {
            const char c = *ci;
            switch (c) {
            case '"': out += "\\\""; break;
            case '\\': out += "\\\\"; break;
            case '\n': out += "\\n"; break;
            case '\r': out += "\\r"; break;
            case '\t': out += "\\t"; break;
            default:
                if (static_cast<unsigned char>(c) < 0x20) {
                    static const char* hex = "0123456789abcdef";
                    out += "\\u00";
                    out.push_back(hex[(static_cast<unsigned char>(c) >> 4) & 0xF]);
                    out.push_back(hex[static_cast<unsigned char>(c) & 0xF]);
                } else {
                    out.push_back(c);
                }
            }
        }
        out.push_back('"');
    }

    void write(std::string& out) const {
        switch (kind_) {
        case Kind::Null: out += "null"; break;
        case Kind::Bool: out += bool_ ? "true" : "false"; break;
        case Kind::Number: out += text_; break;
        case Kind::String: quote(out, text_); break;
        case Kind::Array: {
            out.push_back('[');
            for (std::size_t i = 0; i < items_.size(); ++i) {
                if (i != 0) {
                    out.push_back(',');
                }
                items_[i].write(out);
            }
            out.push_back(']');
            break;
        }
        case Kind::Object: {
            out.push_back('{');
            for (std::size_t i = 0; i < fields_.size(); ++i) {
                if (i != 0) {
                    out.push_back(',');
                }
                quote(out, fields_[i].first);
                out.push_back(':');
                fields_[i].second.write(out);
            }
            out.push_back('}');
            break;
        }
        }
    }

    Kind::Value kind_;
    bool bool_;
    std::string text_;
    std::vector<Json> items_;
    std::vector<std::pair<std::string, Json> > fields_;
};

}  // namespace detail
}  // namespace xtypes
}  // namespace core
}  // namespace dds

#endif  // ZERODDS_DDS_CORE_XTYPES_DETAIL_JSON_HPP
