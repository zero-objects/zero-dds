/* A Cyclone DDS backend of zerodds_test::Calc::add for
 * crates/corba-dds-bridge/tests/omniorb_e2e.rs (zerodds-corba-bridge-1.0
 * §12.3): `calc_backend DOMAIN COUNT` takes the requests of the topic
 * Calc_add_Request (type CalcDds::AddRequest) and answers each on
 * Calc_add_Reply (CalcDds::AddReply) with a + b, both in XCDR1; it prints
 * `ready` once its endpoints exist and `served N` after COUNT requests or
 * 30 s (backend.h). The types come from CalcDds.idl through idlc. */
#include "backend.h"
#include "CalcDds.h"

static dds_return_t answer(dds_entity_t writer, const void *request)
{
  const CalcDds_AddRequest *r = request;
  CalcDds_AddReply reply = { .request_id = r->request_id, .reply_status = 0, .reserved = 0, .result = r->a + r->b };
  return dds_write(writer, &reply);
}

int main(int argc, char **argv)
{
  return backend_serve(argc, argv, &CalcDds_AddRequest_desc, "Calc_add_Request",
                       &CalcDds_AddReply_desc, "Calc_add_Reply", answer);
}
