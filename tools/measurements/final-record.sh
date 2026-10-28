set -x
cd /root/zd-record
tools/interop-results/record.sh --vendors cyclone,rti,fastdds,opendds --cases @/root/zd-record/new-cases.txt; echo STEP_VENDORS=$?
tools/interop-results/record.sh --orbs;     echo STEP_ORBS=$?
tools/interop-results/record.sh --ros;      echo STEP_ROS=$?
tools/interop-results/record.sh --warnings; echo STEP_WARNINGS=$?
tools/interop-results/record.sh --read;     echo STEP_READ=$?
tools/interop-results/regenerate.sh;        echo STEP_REGEN=$?
echo FINAL_RECORD_DONE
