#!/usr/bin/env bash
set -euo pipefail

PORT=3001
HOST="127.0.0.1"
BASE_URL="http://${HOST}:${PORT}"
WORK_DIR="target/manual_test"
DB_PATH="${WORK_DIR}/manual_test.db"
COOKIE_JAR="${WORK_DIR}/cookies.txt"
LOG_FILE="${WORK_DIR}/server.log"

rm -rf "${WORK_DIR}"
mkdir -p "${WORK_DIR}"

echo "========================================================"
echo "          MANTIS-BT MVC MANUAL TEST SUITE               "
echo "========================================================"
echo "Starting test server on ${BASE_URL} with DB ${DB_PATH}..."

MANTIS_ADDR="${HOST}:${PORT}" MANTIS_DB_PATH="${DB_PATH}" ./target/debug/rust-mantis-bt > "${LOG_FILE}" 2>&1 &
SERVER_PID=$!

cleanup() {
    echo ""
    echo "Terminating test server (PID: ${SERVER_PID})..."
    kill "${SERVER_PID}" 2>/dev/null || true
    wait "${SERVER_PID}" 2>/dev/null || true
    echo "Cleanup complete."
}
trap cleanup EXIT

# Wait for server to start
for i in {1..30}; do
    if curl -s "${BASE_URL}/login" > /dev/null 2>&1; then
        echo "Server is UP and responding!"
        break
    fi
    sleep 0.2
done

pass_count=0
fail_count=0

assert_contains() {
    local text="$1"
    local expected="$2"
    local step_name="$3"
    if [[ "$text" == *"$expected"* ]]; then
        echo "  [PASS] $step_name"
        pass_count=$((pass_count + 1))
    else
        echo "  [FAIL] $step_name (Expected '$expected' in output)"
        fail_count=$((fail_count + 1))
    fi
}

assert_status() {
    local actual="$1"
    local expected="$2"
    local step_name="$3"
    if [[ "$actual" == "$expected" ]]; then
        echo "  [PASS] $step_name (HTTP $actual)"
        pass_count=$((pass_count + 1))
    else
        echo "  [FAIL] $step_name (Expected HTTP $expected, got $actual)"
        fail_count=$((fail_count + 1))
    fi
}

# --- TEST SCENARIO 1: Unauthenticated Access ---
echo ""
echo "--- Scenario 1: Unauthenticated Guard ---"
RESP_CODE=$(curl -s -o /dev/null -w "%{http_code}" "${BASE_URL}/account_page")
assert_status "$RESP_CODE" "303" "Unauthenticated request to /account_page redirects to login"

LOGIN_HTML=$(curl -s -L "${BASE_URL}/account_page")
assert_contains "$LOGIN_HTML" "Login" "Redirect landing page contains login form"

# --- TEST SCENARIO 2: Login Flow ---
echo ""
echo "--- Scenario 2: Login Authentication ---"
LOGIN_POST=$(curl -s -c "${COOKIE_JAR}" -b "${COOKIE_JAR}" -X POST "${BASE_URL}/login" \
    -d "username=administrator&password=root" -w "\nHTTP_STATUS:%{http_code}")
assert_contains "$LOGIN_POST" "HTTP_STATUS:303" "POST /login returns 303 Redirect on valid credentials"

AUTH_PAGE=$(curl -s -b "${COOKIE_JAR}" "${BASE_URL}/manage_proj_page")
assert_contains "$AUTH_PAGE" "Manage Projects" "Admin session accesses /manage_proj_page successfully"
assert_contains "$AUTH_PAGE" "administrator" "Navbar reflects logged in user 'administrator'"

# --- TEST SCENARIO 3: Account Page ---
echo ""
echo "--- Scenario 3: Account Profile ---"
ACCOUNT_HTML=$(curl -s -b "${COOKIE_JAR}" "${BASE_URL}/account_page")
assert_contains "$ACCOUNT_HTML" "Edit Account" "GET /account_page renders account edit form"

UPDATE_ACCOUNT=$(curl -s -b "${COOKIE_JAR}" -X POST "${BASE_URL}/account_update" \
    -d "realname=Mantis+Superadmin&email=admin@mantis.local" -w "\nHTTP_STATUS:%{http_code}")
assert_contains "$UPDATE_ACCOUNT" "HTTP_STATUS:303" "POST /account_update redirects on success"

ACCOUNT_HTML_UPDATED=$(curl -s -b "${COOKIE_JAR}" "${BASE_URL}/account_page")
assert_contains "$ACCOUNT_HTML_UPDATED" "Mantis Superadmin" "Updated realname displayed on account page"

# --- TEST SCENARIO 4: Project Management (Create Project, Version, Category) ---
echo ""
echo "--- Scenario 4: Project Management ---"
CREATE_PROJ=$(curl -s -b "${COOKIE_JAR}" -X POST "${BASE_URL}/manage_proj_create" \
    -d "name=Alpha+Core&status=10&view_state=10&description=Primary+product+suite" -w "\nHTTP_STATUS:%{http_code}")
assert_contains "$CREATE_PROJ" "HTTP_STATUS:303" "POST /manage_proj_create creates project and redirects"

PROJ_LIST=$(curl -s -b "${COOKIE_JAR}" "${BASE_URL}/manage_proj_page")
assert_contains "$PROJ_LIST" "Alpha Core" "Project 'Alpha Core' appears in project list"

# Add version to Project 1
ADD_VER=$(curl -s -b "${COOKIE_JAR}" -X POST "${BASE_URL}/manage_proj_ver_add/1" \
    -d "version=v1.0.0&description=Initial+Release" -w "\nHTTP_STATUS:%{http_code}")
assert_contains "$ADD_VER" "HTTP_STATUS:303" "POST /manage_proj_ver_add/1 adds version v1.0.0"

# Add Category to Project 1
ADD_CAT=$(curl -s -b "${COOKIE_JAR}" -X POST "${BASE_URL}/manage_proj_cat_add" \
    -d "project_id=1&name=Security" -w "\nHTTP_STATUS:%{http_code}")
assert_contains "$ADD_CAT" "HTTP_STATUS:303" "POST /manage_proj_cat_add adds category 'Security'"

# --- TEST SCENARIO 5: User Management ---
echo ""
echo "--- Scenario 5: User Management ---"
CREATE_USER=$(curl -s -b "${COOKIE_JAR}" -X POST "${BASE_URL}/manage_user_create" \
    -d "username=dev_bob&realname=Bob+Developer&email=bob@mantis.local&password=password123&access_level=55" \
    -w "\nHTTP_STATUS:%{http_code}")
assert_contains "$CREATE_USER" "HTTP_STATUS:303" "POST /manage_user_create creates developer 'dev_bob'"

USER_LIST=$(curl -s -b "${COOKIE_JAR}" "${BASE_URL}/manage_user_page")
assert_contains "$USER_LIST" "dev_bob" "User 'dev_bob' listed in manage users page"
assert_contains "$USER_LIST" "developer" "User 'dev_bob' has developer access level badge"

# --- TEST SCENARIO 6: Issue Lifecycle (Report, View, Bugnote, Status Change) ---
echo ""
echo "--- Scenario 6: Issue Reporting & Workflow ---"
# Set active project
curl -s -c "${COOKIE_JAR}" -b "${COOKIE_JAR}" -X POST "${BASE_URL}/set_project" -d "project_id=1" > /dev/null

REPORT_PAGE=$(curl -s -b "${COOKIE_JAR}" "${BASE_URL}/bug_report_page")
assert_contains "$REPORT_PAGE" "Enter Issue Details" "GET /bug_report_page renders issue form"

# Submit bug report
REPORT_BUG=$(curl -s -b "${COOKIE_JAR}" -X POST "${BASE_URL}/bug_report" \
    -d "project_id=1&category_id=2&reproducibility=70&severity=60&priority=40&summary=Memory+leak+in+auth+cache&description=Reproducible+under+high+load&version=v1.0.0" \
    -w "\nHTTP_STATUS:%{http_code}")
assert_contains "$REPORT_BUG" "HTTP_STATUS:303" "POST /bug_report creates bug and redirects"

BUG_VIEW=$(curl -s -b "${COOKIE_JAR}" "${BASE_URL}/view_bug_page/1")
assert_contains "$BUG_VIEW" "Memory leak in auth cache" "GET /view_bug_page/1 displays bug summary"
assert_contains "$BUG_VIEW" "Security" "Bug view displays correct category"
assert_contains "$BUG_VIEW" "new" "Bug view displays initial 'new' status"

# Add bugnote
ADD_NOTE=$(curl -s -b "${COOKIE_JAR}" -X POST "${BASE_URL}/bugnote_add/1" \
    -d "note=Investigating+profiler+dumps+now.&view_state=10&time_tracking=25" \
    -w "\nHTTP_STATUS:%{http_code}")
assert_contains "$ADD_NOTE" "HTTP_STATUS:303" "POST /bugnote_add/1 adds note with time tracking"

BUG_VIEW_NOTE=$(curl -s -b "${COOKIE_JAR}" "${BASE_URL}/view_bug_page/1")
assert_contains "$BUG_VIEW_NOTE" "Investigating profiler dumps now." "Bug view displays added bugnote"

# Assign to dev_bob (ID 2)
ASSIGN_BUG=$(curl -s -b "${COOKIE_JAR}" -X POST "${BASE_URL}/bug_assign/1" \
    -d "handler_id=2" -w "\nHTTP_STATUS:%{http_code}")
assert_contains "$ASSIGN_BUG" "HTTP_STATUS:303" "POST /bug_assign/1 assigns issue to dev_bob"

# Change status to resolved (80)
RESOLVE_BUG=$(curl -s -b "${COOKIE_JAR}" -X POST "${BASE_URL}/bug_change_status/1" \
    -d "new_status=80&resolution=20" -w "\nHTTP_STATUS:%{http_code}")
assert_contains "$RESOLVE_BUG" "HTTP_STATUS:303" "POST /bug_change_status/1 marks issue as resolved"

BUG_VIEW_RESOLVED=$(curl -s -b "${COOKIE_JAR}" "${BASE_URL}/view_bug_page/1")
assert_contains "$BUG_VIEW_RESOLVED" "resolved" "Bug view displays updated 'resolved' status"

# --- TEST SCENARIO 7: Tags & Custom Fields ---
echo ""
echo "--- Scenario 7: Tags & Custom Fields ---"
# Create tag
CREATE_TAG=$(curl -s -b "${COOKIE_JAR}" -X POST "${BASE_URL}/tag_create" \
    -d "name=performance&description=Performance+and+resource+utilization" -w "\nHTTP_STATUS:%{http_code}")
assert_contains "$CREATE_TAG" "HTTP_STATUS:303" "POST /tag_create creates tag 'performance'"

# Attach tag to bug 1
ATTACH_TAG=$(curl -s -b "${COOKIE_JAR}" -X POST "${BASE_URL}/bug_tag_attach/1" \
    -d "tag_string=performance" -w "\nHTTP_STATUS:%{http_code}")
assert_contains "$ATTACH_TAG" "HTTP_STATUS:303" "POST /bug_tag_attach/1 attaches tag to bug"

BUG_VIEW_TAG=$(curl -s -b "${COOKIE_JAR}" "${BASE_URL}/view_bug_page/1")
assert_contains "$BUG_VIEW_TAG" "performance" "Bug view displays attached tag 'performance'"

# Create Custom Field
CREATE_CF=$(curl -s -b "${COOKIE_JAR}" -X POST "${BASE_URL}/manage_custom_field_create" \
    -d "name=Environment&field_type=0&possible_values=Staging|Prod|Dev&default_value=Staging&display_report=1&display_update=1" \
    -w "\nHTTP_STATUS:%{http_code}")
assert_contains "$CREATE_CF" "HTTP_STATUS:303" "POST /manage_custom_field_create creates custom field"

CF_PAGE=$(curl -s -b "${COOKIE_JAR}" "${BASE_URL}/manage_custom_field_page")
assert_contains "$CF_PAGE" "Environment" "Custom field 'Environment' listed in manage page"

# Link Custom Field to Project 1
LINK_CF=$(curl -s -b "${COOKIE_JAR}" -X POST "${BASE_URL}/manage_custom_field_proj_add/1" \
    -d "project_id=1&sequence=1" -w "\nHTTP_STATUS:%{http_code}")
assert_contains "$LINK_CF" "HTTP_STATUS:303" "POST /manage_custom_field_proj_add/1 links field to project"

# --- TEST SCENARIO 8: Planning, News, Time Tracking, CSV Export ---
echo ""
echo "--- Scenario 8: Planning, News & Reports ---"
# Roadmap & Summary
ROADMAP_HTML=$(curl -s -b "${COOKIE_JAR}" "${BASE_URL}/roadmap_page")
assert_contains "$ROADMAP_HTML" "Roadmap" "GET /roadmap_page loads successfully"

SUMMARY_HTML=$(curl -s -b "${COOKIE_JAR}" "${BASE_URL}/summary_page")
assert_contains "$SUMMARY_HTML" "Summary Statistics" "GET /summary_page renders statistics"
assert_contains "$SUMMARY_HTML" "Total Issues: 1" "Summary statistics reflects 1 total issue"

BILLING_HTML=$(curl -s -b "${COOKIE_JAR}" "${BASE_URL}/billing_page")
assert_contains "$BILLING_HTML" "Time Tracking / Billing" "GET /billing_page renders time tracking"
assert_contains "$BILLING_HTML" "25 mins" "Billing page records 25 minutes logged on bugnote"

# News Announcement
POST_NEWS=$(curl -s -b "${COOKIE_JAR}" -X POST "${BASE_URL}/news_add" \
    -d "headline=Alpha+v1.0.0+Freeze&body=Code+freeze+is+in+effect+for+v1.0.0.&announcement=1&view_state=10" \
    -w "\nHTTP_STATUS:%{http_code}")
assert_contains "$POST_NEWS" "HTTP_STATUS:303" "POST /news_add posts announcement"

NEWS_PAGE=$(curl -s -b "${COOKIE_JAR}" "${BASE_URL}/news_page")
assert_contains "$NEWS_PAGE" "Alpha v1.0.0 Freeze" "News page displays posted headline"
assert_contains "$NEWS_PAGE" "Announcement" "News page renders announcement badge"

# CSV Export
CSV_CONTENT=$(curl -s -b "${COOKIE_JAR}" "${BASE_URL}/csv_export")
assert_contains "$CSV_CONTENT" "Id,Project,Reporter,Assigned To" "GET /csv_export returns proper CSV header"
assert_contains "$CSV_CONTENT" "Memory leak in auth cache" "CSV export contains reported bug summary"

# --- TEST SCENARIO 9: REST API & API Tokens ---
echo ""
echo "--- Scenario 9: REST API & API Tokens ---"
CREATE_TOKEN_RESP=$(curl -s -i -b "${COOKIE_JAR}" -d "name=Test+API+Token" "${BASE_URL}/api_token_create")
assert_contains "$CREATE_TOKEN_RESP" "303" "POST /api_token_create redirects with created token"

RAW_TOKEN=$(echo "$CREATE_TOKEN_RESP" | grep -i "location:" | grep -oE 'token=[a-zA-Z0-9_-]+' | cut -d'=' -f2 | tr -d '\r\n' || true)
echo "  [INFO] Extracted API Token: ${RAW_TOKEN}"

TOKEN_PAGE=$(curl -s -b "${COOKIE_JAR}" "${BASE_URL}/api_tokens_page?token=${RAW_TOKEN}")
assert_contains "$TOKEN_PAGE" "New API Token Created!" "API tokens page renders new token banner"

if [[ -n "$RAW_TOKEN" ]]; then
    ME_JSON=$(curl -s -H "Authorization: ${RAW_TOKEN}" "${BASE_URL}/api/rest/users/me")
    assert_contains "$ME_JSON" '"name":"administrator"' "GET /api/rest/users/me returns authenticated user details"
    assert_contains "$ME_JSON" '"access_level"' "GET /api/rest/users/me returns access level object"

    PROJECTS_JSON=$(curl -s -H "Authorization: ${RAW_TOKEN}" "${BASE_URL}/api/rest/projects")
    assert_contains "$PROJECTS_JSON" '"name":"Alpha Core"' "GET /api/rest/projects returns 'Alpha Core'"

    # Create Issue via REST API
    CREATE_REST_ISSUE=$(curl -s -H "Authorization: ${RAW_TOKEN}" -H "Content-Type: application/json" \
        -X POST "${BASE_URL}/api/rest/issues" \
        -d '{"summary":"Bug reported via REST API","project":{"id":1},"category":{"id":1}}')
    assert_contains "$CREATE_REST_ISSUE" '"summary":"Bug reported via REST API"' "POST /api/rest/issues creates issue via API"

    ISSUES_JSON=$(curl -s -H "Authorization: ${RAW_TOKEN}" "${BASE_URL}/api/rest/issues")
    assert_contains "$ISSUES_JSON" "Bug reported via REST API" "GET /api/rest/issues lists newly created issue"
else
    echo "  [FAIL] Could not extract raw token for REST API test"
    fail_count=$((fail_count + 1))
fi

# --- TEST SCENARIO 10: Logout ---
echo ""
echo "--- Scenario 10: Logout Verification ---"
LOGOUT_RESP=$(curl -s -c "${COOKIE_JAR}" -b "${COOKIE_JAR}" -X POST "${BASE_URL}/logout" -w "\nHTTP_STATUS:%{http_code}")
assert_contains "$LOGOUT_RESP" "HTTP_STATUS:303" "POST /logout redirects to /login"

AFTER_LOGOUT_CODE=$(curl -s -b "${COOKIE_JAR}" -o /dev/null -w "%{http_code}" "${BASE_URL}/account_page")
assert_status "$AFTER_LOGOUT_CODE" "303" "After logout, access to /account_page is redirected (unauthenticated)"

echo ""
echo "========================================================"
echo "MANUAL TEST SUMMARY: Passed: ${pass_count}, Failed: ${fail_count}"
echo "========================================================"

if [[ $fail_count -eq 0 ]]; then
    exit 0
else
    exit 1
fi
