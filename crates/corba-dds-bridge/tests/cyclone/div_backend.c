/* A Cyclone DDS backend of zerodds_test::Divider::divide for
 * crates/corba-dds-bridge/tests/omniorb_e2e.rs (zerodds-corba-bridge-1.0
 * §5.2): `div_backend DOMAIN COUNT`. Its types are the ones the bridge
 * generated from tests/omniorb/Div.idl into corba.idl_dir (Div.idl there),
 * through idlc. It takes the requests of the topic Div_Request and answers
 * each on Div_Reply in XCDR1: quotient and remainder, the exception
 * DivideByZero with its members for a denominator of 0, the exception
 * Unsupported for a negative numerator. It prints `ready` once its
 * endpoints exist and `served N` after COUNT requests or 30 s (backend.h). */
#include "backend.h"
#include "Div.h"

typedef zerodds_test_Divider_divide_Request Request;
typedef zerodds_test_Divider_divide_Reply Reply;

static dds_return_t answer(dds_entity_t writer, const void *request)
{
  const Request *r = request;
  Reply reply = { .request_id = r->request_id };
  if (r->f.denominator == 0) {
    reply.status._d = 1;
    reply.status._u.raised._d = zerodds_test_Divider_divide_zerodds_test_Divider_DivideByZero;
    reply.status._u.raised._u.as_zerodds_test_Divider_DivideByZero.numerator = r->f.numerator;
    reply.status._u.raised._u.as_zerodds_test_Divider_DivideByZero.why = "no denominator";
  } else if (r->f.numerator < 0) {
    reply.status._d = 1;
    reply.status._u.raised._d = zerodds_test_Divider_divide_zerodds_test_Divider_Unsupported;
  } else {
    reply.status._d = 0;
    reply.status._u.returned.reserved = 0;
    reply.status._u.returned.return_value = r->f.numerator / r->f.denominator;
    reply.status._u.returned.remainder = r->f.numerator % r->f.denominator;
  }
  return dds_write(writer, &reply);
}

int main(int argc, char **argv)
{
  return backend_serve(argc, argv, &zerodds_test_Divider_divide_Request_desc, "Div_Request",
                       &zerodds_test_Divider_divide_Reply_desc, "Div_Reply", answer);
}
